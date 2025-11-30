pub fn parse_num_worker_threads(s: &str) -> Result<usize, String> {
    if s == "auto" {
        let n = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        Ok(n)
    } else {
        s.parse::<usize>().map_err(|_| format!("{}", s))
    }
}
