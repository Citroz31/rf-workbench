//! Model-specific DC controls. References: Keysight E3631-90002 and E36311-90008.
//! Enable is explicit; Apply switches off the affected output before setting it.
use crate::{Error, Result, Session};
use rf_core::{Config, Kind};
use std::sync::atomic::{AtomicBool, Ordering};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Read,
    Apply,
    Enable,
    Disable,
}
#[derive(Clone, Debug)]
pub struct Reading {
    pub voltage_v: f64,
    pub current_a: f64,
    pub enabled: bool,
}
pub fn model(kind: Kind) -> &'static str {
    if kind == Kind::DcSupplyE3631A {
        "E3631A"
    } else {
        "E36313A"
    }
}
pub fn identifies(kind: Kind, idn: &str) -> bool {
    let fields: Vec<_> = idn.trim().split(',').collect();
    fields.len() >= 2
        && ["KEYSIGHT", "AGILENT", "HEWLETT-PACKARD"]
            .iter()
            .any(|s| fields[0].to_ascii_uppercase().contains(s))
        && fields[1].trim().eq_ignore_ascii_case(model(kind))
}
fn channel_name(kind: Kind, ch: u8) -> String {
    if kind == Kind::DcSupplyE3631A {
        ["P6V", "P25V", "N25V"][(ch - 1) as usize].into()
    } else {
        format!("CH{ch}")
    }
}
fn number(s: String) -> Result<f64> {
    s.trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| Error::Protocol("Lecture DC invalide".into()))
}
fn state(s: String) -> Result<bool> {
    match s.trim().to_ascii_uppercase().as_str() {
        "0" | "OFF" => Ok(false),
        "1" | "ON" => Ok(true),
        _ => Err(Error::Protocol("État de sortie DC invalide".into())),
    }
}
fn no_error(s: &mut dyn Session) -> Result<()> {
    let e = s.query("SYST:ERR?")?;
    if e.trim()
        .split(',')
        .next()
        .is_some_and(|v| v.trim().parse::<i32>() == Ok(0))
    {
        Ok(())
    } else {
        Err(Error::Protocol(format!("Erreur SCPI alimentation : {e}")))
    }
}
pub fn disable(s: &mut dyn Session, kind: Kind, ch: u8) -> Result<()> {
    if kind == Kind::DcSupplyE3631A {
        s.write("OUTP OFF")
    } else {
        s.write(&format!("OUTP OFF,(@{ch})"))
    }
}
pub fn execute(
    s: &mut dyn Session,
    kind: Kind,
    c: &Config,
    action: Action,
    cancel: &AtomicBool,
) -> Result<Reading> {
    if !kind.is_dc_supply() || !(1..=3).contains(&c.instrument.dc.channel) {
        return Err(Error::Protocol("Canal DC invalide".into()));
    }
    let dc = &c.instrument.dc;
    let ch = dc.channel;
    if !identifies(kind, &s.query("*IDN?")?) {
        return Err(Error::Protocol(
            "Modèle DC différent du bloc ; aucune consigne envoyée".into(),
        ));
    }
    if matches!(action, Action::Apply | Action::Enable) {
        dc.validate(kind).map_err(Error::Protocol)?;
        no_error(s)?;
        if kind == Kind::DcSupplyE3631A {
            if state(s.query("OUTP:TRAC?")?)? {
                return Err(Error::Protocol(
                    "Désactiver le suivi ±25 V sur l'alimentation avant une consigne indépendante"
                        .into(),
                ));
            }
        } else {
            if s.query("OUTP:PAIR?")?.trim().trim_matches('"') != "OFF" {
                return Err(Error::Protocol(
                    "Utiliser le mode indépendant (PAIR OFF) de l'E36313A".into(),
                ));
            }
            if s.query("OUTP:STAT:COUP:CHAN?")?.trim().trim_matches('"') != "NONE" {
                return Err(Error::Protocol(
                    "Sorties couplées : désactiver le couplage sur l'appareil".into(),
                ));
            }
            if s.query(&format!("VOLT:MODE? (@{ch})"))?.trim() != "FIX"
                || s.query(&format!("CURR:MODE? (@{ch})"))?.trim() != "FIX"
            {
                return Err(Error::Protocol(
                    "Le pilote DC exige les modes tension/courant FIX".into(),
                ));
            }
        }
    }
    let check = || {
        if cancel.load(Ordering::Acquire) {
            Err(Error::Protocol("Commande DC arrêtée".into()))
        } else {
            Ok(())
        }
    };
    match action {
        Action::Apply => {
            check()?;
            disable(s, kind, ch)?;
            if kind == Kind::DcSupplyE3631A {
                s.write(&format!("INST {}", channel_name(kind, ch)))?;
                check()?;
                s.write(&format!("CURR {}", dc.current_limit_a))?;
                check()?;
                s.write(&format!("VOLT {}", dc.voltage_v))?;
            } else {
                check()?;
                s.write(&format!("CURR {},(@{ch})", dc.current_limit_a))?;
                check()?;
                s.write(&format!("VOLT {},(@{ch})", dc.voltage_v))?;
            }
            no_error(s)?;
        }
        Action::Enable => {
            if kind == Kind::DcSupplyE3631A {
                s.write(&format!("INST {}", channel_name(kind, ch)))?;
            }
            let suffix = if kind == Kind::DcSupplyE3631A {
                String::new()
            } else {
                format!(" (@{ch})")
            };
            let voltage = number(s.query(&format!("VOLT?{suffix}"))?)?;
            let current = number(s.query(&format!("CURR?{suffix}"))?)?;
            if (voltage - dc.voltage_v).abs() > 1e-5 || (current - dc.current_limit_a).abs() > 1e-5
            {
                return Err(Error::Protocol(
                    "Consignes appareil différentes : appliquer les consignes avant ON".into(),
                ));
            }
            check()?;
            if kind == Kind::DcSupplyE3631A {
                s.write("OUTP ON")?;
            } else {
                s.write(&format!("OUTP ON,(@{ch})"))?;
            }
            no_error(s)?;
        }
        Action::Disable => {
            disable(s, kind, ch)?;
            no_error(s)?;
        }
        Action::Read => {}
    }
    let suffix = if kind == Kind::DcSupplyE3631A {
        format!(" {}", channel_name(kind, ch))
    } else {
        format!(" (@{ch})")
    };
    let voltage_v = number(s.query(&format!("MEAS:VOLT?{suffix}"))?)?;
    let current_a = number(s.query(&format!("MEAS:CURR?{suffix}"))?)?;
    let suffix = if kind == Kind::DcSupplyE3631A {
        String::new()
    } else {
        format!(" (@{ch})")
    };
    let enabled = state(s.query(&format!("OUTP?{suffix}"))?)?;
    Ok(Reading {
        voltage_v,
        current_a,
        enabled,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Mock {
        writes: Vec<String>,
        idn: String,
        on: bool,
        voltage: f64,
        current: f64,
        coupled: bool,
    }
    impl Session for Mock {
        fn write(&mut self, c: &str) -> Result<()> {
            self.writes.push(c.into());
            if c.starts_with("OUTP ON") {
                self.on = true;
            }
            if c.starts_with("OUTP OFF") {
                self.on = false;
            }
            Ok(())
        }
        fn read_binary(&mut self, _: &str) -> Result<Vec<u8>> {
            unreachable!()
        }
        fn query(&mut self, c: &str) -> Result<String> {
            Ok(match c {
                "*IDN?" => self.idn.clone(),
                "SYST:ERR?" => "0,No error".into(),
                "OUTP:PAIR?" => "OFF".into(),
                "OUTP:TRAC?" => "0".into(),
                "OUTP:STAT:COUP:CHAN?" => if self.coupled { "1,2" } else { "NONE" }.into(),
                c if c.starts_with("VOLT:MODE?") || c.starts_with("CURR:MODE?") => "FIX".into(),
                c if c.starts_with("MEAS:VOLT?") || c.starts_with("VOLT?") => {
                    self.voltage.to_string()
                }
                c if c.starts_with("MEAS:CURR?") || c.starts_with("CURR?") => {
                    self.current.to_string()
                }
                c if c.starts_with("OUTP?") => (self.on as u8).to_string(),
                _ => return Err(Error::Protocol(format!("Unexpected {c}"))),
            })
        }
    }
    #[test]
    fn apply_never_enables_and_wrong_model_never_writes() {
        let c = Config::default();
        let mut s = Mock {
            idn: "Keysight,E3631A,123,1".into(),
            ..Default::default()
        };
        execute(
            &mut s,
            Kind::DcSupplyE3631A,
            &c,
            Action::Apply,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(s.writes, ["OUTP OFF", "INST P6V", "CURR 0.1", "VOLT 0"]);
        s.writes.clear();
        assert!(
            execute(
                &mut s,
                Kind::DcSupplyE36313A,
                &c,
                Action::Enable,
                &AtomicBool::new(false)
            )
            .is_err()
        );
        assert!(s.writes.is_empty());
    }
    #[test]
    fn independent_channel_enable_checks_actual_setpoints_and_stop() {
        let mut c = Config::default();
        c.instrument.dc.channel = 2;
        c.instrument.dc.voltage_v = 1.5;
        let mut s = Mock {
            idn: "Keysight,E36313A,123,1".into(),
            voltage: 1.5,
            current: 0.1,
            ..Default::default()
        };
        assert!(
            execute(
                &mut s,
                Kind::DcSupplyE36313A,
                &c,
                Action::Enable,
                &AtomicBool::new(false)
            )
            .unwrap()
            .enabled
        );
        assert_eq!(s.writes, ["OUTP ON,(@2)"]);
        disable(&mut s, Kind::DcSupplyE36313A, 2).unwrap();
        assert!(!s.on);
        s.writes.clear();
        s.coupled = true;
        assert!(
            execute(
                &mut s,
                Kind::DcSupplyE36313A,
                &c,
                Action::Enable,
                &AtomicBool::new(false)
            )
            .is_err()
        );
        assert!(s.writes.is_empty());
        s.coupled = false;
        s.voltage = 20.;
        assert!(
            execute(
                &mut s,
                Kind::DcSupplyE36313A,
                &c,
                Action::Enable,
                &AtomicBool::new(false)
            )
            .is_err()
        );
        assert!(s.writes.is_empty());
        s.voltage = 1.5;
        assert!(
            execute(
                &mut s,
                Kind::DcSupplyE36313A,
                &c,
                Action::Enable,
                &AtomicBool::new(true)
            )
            .is_err()
        );
        assert!(s.writes.is_empty());
    }
}
