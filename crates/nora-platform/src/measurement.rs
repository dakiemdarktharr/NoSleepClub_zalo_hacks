//! Optional local measurement of the actual NORA Job Object process set.
//! Working sets are summed per sampling pass, never by summing lifetime peaks.
use crate::{BudgetJob, RAM_LIMIT_BYTES, process_memory};
use serde::Serialize;
use std::{
    collections::BTreeSet,
    fs::File,
    io,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Default, Serialize)]
pub struct MemoryMeasurement {
    pub schema_version: u32,
    pub scope: &'static str,
    pub sampling_interval_ms: u32,
    pub samples: u64,
    pub elapsed_ms: u64,
    pub sampled_peak_total_working_set_bytes: u64,
    pub sampled_peak_total_private_commit_bytes: u64,
    pub peak_job_commit_counter_bytes: u64,
    pub peak_concurrent_processes: usize,
    pub observed_process_ids: BTreeSet<u32>,
    pub counter_errors: u64,
    pub sampled_ram_within_limit: bool,
}
pub struct Measurement {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<io::Result<MemoryMeasurement>>>,
}
impl Measurement {
    pub fn start(job: Arc<BudgetJob>, path: &Path) -> io::Result<Self> {
        // Reserve a new diagnostic destination before launching the worker. No
        // credentials, prompts, paths or raw API output enter this report.
        let file = File::create_new(path)?;
        drop(file);
        let path: PathBuf = path.to_owned();
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let worker=thread::Builder::new().name("nora-memory-meter".into()).spawn(move||{
            let started=Instant::now();let mut report=MemoryMeasurement{schema_version:1,scope:"Current process set from NORA Job Object; external approved user applications excluded; GPU measured separately. Sampling can miss short-lived peaks.",sampling_interval_ms:20,..Default::default()};
            loop{
                match job.snapshot(){Ok(job)=>{report.peak_job_commit_counter_bytes=report.peak_job_commit_counter_bytes.max(job.peak_job_commit_bytes);report.peak_concurrent_processes=report.peak_concurrent_processes.max(job.process_ids.len());let mut ws=0u64;let mut commit=0u64;
                    for pid in job.process_ids{report.observed_process_ids.insert(pid);match process_memory(pid){Ok(m)=>{ws=ws.saturating_add(m.working_set_bytes);commit=commit.saturating_add(m.private_commit_bytes);},Err(e) if e.raw_os_error()==Some(87)=>{},Err(_)=>report.counter_errors+=1}}
                    report.sampled_peak_total_working_set_bytes=report.sampled_peak_total_working_set_bytes.max(ws);report.sampled_peak_total_private_commit_bytes=report.sampled_peak_total_private_commit_bytes.max(commit);report.samples+=1;
                },Err(_)=>report.counter_errors+=1}
                report.elapsed_ms=started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
                report.sampled_ram_within_limit=report.samples>0&&report.counter_errors==0&&report.sampled_peak_total_working_set_bytes<=RAM_LIMIT_BYTES&&report.sampled_peak_total_private_commit_bytes<=RAM_LIMIT_BYTES;
                if report.samples.is_multiple_of(50)||signal.load(Ordering::Acquire){let file=File::create(&path)?;serde_json::to_writer(&file,&report)?;file.sync_all()?;}
                if signal.load(Ordering::Acquire){return Ok(report);}thread::sleep(Duration::from_millis(20));
            }
        })?;
        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }
    pub fn finish(mut self) -> io::Result<MemoryMeasurement> {
        self.stop.store(true, Ordering::Release);
        self.worker
            .take()
            .ok_or_else(|| io::Error::other("meter already finished"))?
            .join()
            .map_err(|_| io::Error::other("memory meter failed"))?
    }
}
impl Drop for Measurement {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
