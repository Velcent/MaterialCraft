//! [`Machine`]: the computer a run happened on. Runs are compared only with runs on the same machine (same host
//! name and CPU).

use materialcraft::json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    pub host: String,
    pub cpu: String,
    /// Logical cores.
    pub cores: usize,
}

impl Machine {
    /// This computer (`unknown` for what can't be found out).
    pub fn this() -> Machine {
        Machine { host: host(), cpu: cpu(), cores: std::thread::available_parallelism().map_or(0, |n| n.get()) }
    }

    pub fn same_as(&self, other: &Machine) -> bool {
        self.host == other.host && self.cpu == other.cpu
    }

    pub fn to_json(&self) -> Value {
        Value::object().with("host", self.host.as_str()).with("cpu", self.cpu.as_str()).with("cores", self.cores)
    }

    pub fn from_json(v: &Value) -> Machine {
        let text = |k: &str| v[k].as_str().unwrap_or("unknown").to_string();
        Machine { host: text("host"), cpu: text("cpu"), cores: v["cores"].as_usize().unwrap_or(0) }
    }
}

fn first_line(path: &str) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines().next().map(|l| l.trim().to_string()).filter(|l| !l.is_empty())
}

fn command(program: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(program).args(args).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !text.is_empty()).then_some(text)
}

fn host() -> String {
    first_line("/proc/sys/kernel/hostname")
        .or_else(|| std::env::var("HOSTNAME").ok().filter(|h| !h.is_empty()))
        .or_else(|| command("hostname", &[]))
        .unwrap_or_else(|| "unknown".into())
}

fn cpu() -> String {
    let linux = std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|t| t.lines().find(|l| l.starts_with("model name")).and_then(|l| l.split_once(':')).map(|(_, v)| v.trim().to_string()));
    linux.or_else(|| command("sysctl", &["-n", "machdep.cpu.brand_string"])).unwrap_or_else(|| "unknown".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let m = Machine { host: "h".into(), cpu: "c".into(), cores: 16 };
        assert_eq!(Machine::from_json(&m.to_json()), m);
        assert!(m.same_as(&Machine { cores: 8, ..m.clone() }));
        assert!(Machine::this().cores > 0);
    }
}
