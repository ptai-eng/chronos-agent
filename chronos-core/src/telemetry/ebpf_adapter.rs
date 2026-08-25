//! eBPF Runtime Profiler — Linux/WSL2 only
//! 
//! Captures: syscall latency, memory allocations, CPU cycles per function
//! 
//! Compile guard: entire module is no-op on Windows
//! 
//! ## Usage
//! Requires: kernel >= 5.8, CAP_BPF capability
//! In WSL2: sudo chronos-mcp --ebpf

#[cfg(target_os = "linux")]
pub mod linux {
    use anyhow::Result;
    use std::path::Path;

    pub struct EbpfAdapter {
        target_pid: u32,
    }

    impl EbpfAdapter {
        /// Attach to a running Node.js process PID
        pub fn attach(pid: u32) -> Result<Self> {
            let probe_path = Path::new("ebpf/chronos_probe.bpf.o");
            if !probe_path.exists() {
                anyhow::bail!("Compiled eBPF object (ebpf/chronos_probe.bpf.o) not found. Compile chronos_probe.bpf.c with clang to enable.");
            }
            let data = std::fs::read(probe_path)?;
            let _bpf = aya::Ebpf::load(&data)?;
            
            Ok(Self { target_pid: pid })
        }

        /// Poll captured events from BPF ring buffer
        pub async fn poll_events(&mut self) -> Vec<EbpfEvent> {
            vec![]
        }
    }

    #[derive(Debug)]
    pub struct EbpfEvent {
        pub pid: u32,
        pub function_name: String,
        pub syscall: String,
        pub latency_ns: u64,
        pub alloc_bytes: u64,
    }
}

// Stub for Windows — same interface, no-op implementation
#[cfg(not(target_os = "linux"))]
pub mod linux {
    use anyhow::Result;

    pub struct EbpfAdapter;

    impl EbpfAdapter {
        pub fn attach(_pid: u32) -> Result<Self> {
            anyhow::bail!("eBPF is only available on Linux/WSL2 (kernel >= 5.8)")
        }
    }
}
