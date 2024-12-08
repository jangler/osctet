# Developer notes

## Profiling on Windows

1. `$ cargo install blondie flamegraph`
2. Run the terminal (or VS Code) as administrator.
3. Turn off real-time protection in the Windows Security settings, under Virus & Threat Protection.
4. `$ DTRACE="C:\Users\USERNAME\.cargo\bin\blondie_dtrace.exe" CARGO_PROFILE_RELEASE_DEBUG=true cargo flamegraph`