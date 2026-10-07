use rf_core::{Graph, Kind};
use rf_runtime::{
    Command, Engine, Event, Worker,
    debug::{BufferData, PREVIEW_SAMPLES, Snapshot},
};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

fn paused(worker: &Worker) -> Snapshot {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        for e in worker.events.try_iter() {
            if let Event::Error(e) = e {
                panic!("{e}");
            }
        }
        if let Some(s) = worker.take_debug()
            && s.paused
        {
            return s;
        }
        assert!(Instant::now() < deadline, "Debugger did not reach a pause");
        std::thread::sleep(Duration::from_millis(2));
    }
}
fn idle(worker: &Worker) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while worker.is_busy() {
        assert!(Instant::now() < deadline, "Debugger did not stop");
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn steps_one_block_then_continues_to_breakpoint_and_finishes() {
    let mut g = Graph::demo();
    g.remove(4);
    g.connect(3, 5).unwrap();
    g.nodes.iter_mut().find(|n| n.id == 6).unwrap().breakpoint = true;
    let w = Worker::spawn(|| {});
    w.submit(Command::Debug {
        graph: g,
        python_path: "unused".into(),
    })
    .unwrap();
    let first = paused(&w);
    assert_eq!(first.node, 1);
    assert!(first.completed.is_empty());
    w.debug_step();
    let second = paused(&w);
    assert_eq!(second.node, 2);
    assert_eq!(second.completed, vec![1]);
    assert_eq!(second.buffers.len(), 1);
    w.debug_continue();
    let limit = paused(&w);
    assert_eq!(limit.node, 6);
    assert_eq!(limit.completed, vec![1, 2, 3, 5]);
    assert!(
        limit
            .buffers
            .iter()
            .any(|b| matches!(b.data.as_ref(), BufferData::Trace(_)))
    );
    w.debug_continue();
    idle(&w);
    assert_eq!(w.take_latest().unwrap().completed, vec![1, 2, 3, 5, 6]);
}
#[test]
fn stop_cancels_a_paused_debugger_and_physical_resources_are_rejected() {
    let w = Worker::spawn(|| {});
    w.submit(Command::Debug {
        graph: Graph::network_demo(),
        python_path: "unused".into(),
    })
    .unwrap();
    paused(&w);
    w.stop();
    idle(&w);
    assert!(w.events.try_iter().any(|e| matches!(e, Event::Error(_))));
    let mut g = Graph::default();
    let id = g.add(Kind::Generator, [0., 0.]);
    g.nodes
        .iter_mut()
        .find(|n| n.id == id)
        .unwrap()
        .config
        .resource = "TCPIP::127.0.0.1::1::SOCKET".into();
    w.submit(Command::Debug {
        graph: g,
        python_path: "unused".into(),
    })
    .unwrap();
    idle(&w);
    assert!(
        w.events
            .try_iter()
            .any(|e| matches!(e,Event::Error(ref s) if s.contains("simulé")))
    );
}
#[test]
fn buffer_previews_are_bounded_and_keep_original_size_and_units() {
    let mut g = Graph::default();
    g.add(Kind::Awg, [0., 0.]);
    g.nodes[0].config.samples = 5000;
    let r = Engine::default()
        .execute(&g, 0, "unused", false, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(r.buffers.len(), 2);
    for b in &r.buffers {
        assert_eq!(b.total_samples, 5000);
        let BufferData::Waveform(w) = b.data.as_ref() else {
            panic!("Expected waveform")
        };
        assert_eq!(w.samples.len(), PREVIEW_SAMPLES);
        assert_eq!(w.unit, "FS");
        assert_eq!(w.sample_rate_hz, 100e6);
        assert!(w.simulated);
    }
}
