use rf_core::dsp::*;
use rf_hal::{
    local_capabilities, queue,
    recording::{Reader, Writer},
    transport::Io,
};
use std::io::{Read, Write};
fn frame() -> IqFrame {
    IqFrame {
        samples: (0..32)
            .map(|i| Complex::new(i as f64 / 16., -0.25))
            .collect(),
        sample_rate: 48000.,
        center_hz: 100e6,
        unit: Unit::Fs,
        simulated: false,
        time: TimeTag {
            first_sample: 123,
            epoch_ns: Some(987654321),
            clock_domain: "test-clock".into(),
            discontinuity: true,
        },
    }
}
#[test]
fn raw_index_roundtrip_and_truncation() {
    let path = std::env::temp_dir().join(format!("rf-hal-{}.cf32", std::process::id()));
    let mut w = Writer::create(&path).unwrap();
    let f = frame();
    w.append(&f).unwrap();
    w.append(&f).unwrap();
    drop(w);
    let mut r = Reader::open(&path).unwrap();
    assert_eq!(r.frames(), 2);
    r.seek_frame(1).unwrap();
    assert_eq!(r.read_frame().unwrap(), f);
    assert!(r.read_frame().is_err());
    drop(r);
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(5)
        .unwrap();
    assert!(Reader::open(&path).is_err());
    std::fs::remove_file(&path).unwrap();
    let mut json = path.into_os_string();
    json.push(".json");
    std::fs::remove_file(json).unwrap();
}
#[test]
fn lock_free_order_and_overflow() {
    let (mut tx, mut rx) = queue::bounded::<usize>(4);
    for i in 0..4 {
        tx.push(i).unwrap();
    }
    assert_eq!(tx.push(4), Err(4));
    assert_eq!(rx.overflows(), 1);
    for i in 0..4 {
        assert_eq!(rx.pop(), Some(i));
    }
    assert_eq!(rx.pop(), None);
    let producer = std::thread::spawn(move || {
        for i in 0..100000 {
            let mut value = i;
            loop {
                match tx.push(value) {
                    Ok(()) => break,
                    Err(v) => {
                        value = v;
                        std::thread::yield_now();
                    }
                }
            }
        }
    });
    for i in 0..100000 {
        loop {
            if let Some(v) = rx.pop() {
                assert_eq!(v, i);
                break;
            }
            std::thread::yield_now();
        }
    }
    producer.join().unwrap();
}
#[test]
fn tcp_fragmented_frame_and_echo() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let f = frame();
    let remote = f.clone();
    let server = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let b = serde_json::to_vec(&remote).unwrap();
        for x in (b.len() as u32).to_be_bytes() {
            s.write_all(&[x]).unwrap();
        }
        for b in b.chunks(11) {
            s.write_all(b).unwrap();
        }
        let mut h = [0; 4];
        s.read_exact(&mut h).unwrap();
        let mut b = vec![0; u32::from_be_bytes(h) as usize];
        s.read_exact(&mut b).unwrap();
        assert_eq!(serde_json::from_slice::<IqFrame>(&b).unwrap(), remote);
    });
    let c = Settings {
        io_backend: "TCP".into(),
        endpoint: addr.to_string(),
        ..Default::default()
    };
    let mut io = Io::open(&c, true).unwrap();
    let received = io.read().unwrap();
    assert_eq!(received, f);
    io.write(&received).unwrap();
    server.join().unwrap();
}
#[test]
fn tcp_timeout_bounds_whole_trickled_header() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        for b in 100u32.to_be_bytes() {
            std::thread::sleep(std::time::Duration::from_millis(35));
            if s.write_all(&[b]).is_err() {
                break;
            }
        }
    });
    let c = Settings {
        io_backend: "TCP".into(),
        endpoint: addr.to_string(),
        timeout_ms: 60,
        ..Default::default()
    };
    let mut io = Io::open(&c, true).unwrap();
    let start = std::time::Instant::now();
    assert!(io.read().is_err());
    assert!(start.elapsed() < std::time::Duration::from_millis(130));
    drop(io);
    server.join().unwrap();
}
#[test]
fn udp_retains_metadata() {
    let socket = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
    let addr = socket.local_addr().unwrap();
    let mut sender = Io::open(
        &Settings {
            io_backend: "UDP".into(),
            endpoint: addr.to_string(),
            ..Default::default()
        },
        false,
    )
    .unwrap();
    let f = frame();
    sender.write(&f).unwrap();
    let mut b = [0; 60000];
    let n = socket.recv(&mut b).unwrap();
    assert_eq!(serde_json::from_slice::<IqFrame>(&b[..n]).unwrap(), f);
}
#[test]
fn capability_checks_refuse_unsupported_sync() {
    let c = Settings {
        clock: "PTP".into(),
        ..Default::default()
    };
    assert!(local_capabilities().check(&c).is_err());
    let c = Settings {
        mimo_channels: 2,
        ..Default::default()
    };
    assert!(local_capabilities().check(&c).is_err());
}
#[test]
fn pump_marks_loss_and_stops_bounded_reader() {
    let mut index = 0;
    let mut pump = rf_hal::streaming::Pump::spawn(
        move || {
            std::thread::sleep(std::time::Duration::from_millis(1));
            let mut f = frame();
            f.time.first_sample = index;
            f.time.discontinuity = false;
            index += 32;
            Ok(f)
        },
        2,
    );
    std::thread::sleep(std::time::Duration::from_millis(30));
    assert!(pump.overflows() > 0);
    let _ = pump.read(std::time::Duration::from_secs(1)).unwrap();
    let _ = pump.read(std::time::Duration::from_secs(1)).unwrap();
    let mut gap = false;
    for _ in 0..10 {
        if pump
            .read(std::time::Duration::from_secs(1))
            .unwrap()
            .time
            .discontinuity
        {
            gap = true;
            break;
        }
    }
    assert!(gap);
    drop(pump);
}
#[test]
fn typed_vna_uses_real_scpi_response() {
    use rf_hal::{Instrument, InstrumentClass, InstrumentProfile};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (s, _) = listener.accept().unwrap();
        let mut s = std::io::BufReader::new(s);
        use std::io::BufRead;
        let mut line = String::new();
        s.read_line(&mut line).unwrap();
        assert_eq!(line, ":DATA?\n");
        s.get_mut().write_all(b"0.1,0,0,0.2\n").unwrap();
    });
    let p = InstrumentProfile {
        name: "VNA TCP test".into(),
        class: InstrumentClass::Vna,
        resource: format!("TCPIP::127.0.0.1::{}::SOCKET", addr.port()),
        capabilities: local_capabilities(),
        query: ":DATA?".into(),
        format: "ascii_ri".into(),
        timeout_ms: 1000,
        reconnects: 0,
    };
    let mut instrument = Instrument::open(p).unwrap();
    assert!(instrument.set_generator(1e9, -10.).is_err());
    let t = instrument.read_vna(vec![1e9, 2e9], "S11").unwrap();
    assert!(!t.simulated);
    assert_eq!(t.phase_deg, [0., 90.]);
    assert!((t.magnitude_db[0] + 20.).abs() < 1e-12);
    server.join().unwrap();
}
