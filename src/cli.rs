#[derive(Clone)]
pub enum NumWorkerThreads {
    Num(usize),
}

impl std::fmt::Debug for NumWorkerThreads {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NumWorkerThreads::Num(n) => write!(f, "Num({})", n),
        }
    }
}

pub fn parse_num_worker_threads(s: &str) -> Result<NumWorkerThreads, String> {
    if s == "auto" {
        let n = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        Ok(NumWorkerThreads::Num(n))
    } else {
        s.parse::<usize>()
            .map(NumWorkerThreads::Num)
            .map_err(|_| format!("Invalid value for --worker-threads: {}", s))
    }
}
