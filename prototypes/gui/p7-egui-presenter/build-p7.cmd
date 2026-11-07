@echo off
rem Builds and checks the P7 prototype only: cargo build (release), cargo test, cargo clippy.
rem Log: D:\PhotoWorkflowData\analysis\gui\p7\build.txt. The app is not touched; nothing is installed.
setlocal
set OUT=D:\PhotoWorkflowData\analysis\gui\p7
if not exist "%OUT%" mkdir "%OUT%"
cd /d "%~dp0"
set T=..\..\..\target\prototypes
echo === build > "%OUT%\build.txt"
cargo build --release --target-dir %T% >> "%OUT%\build.txt" 2>&1
echo build EXIT %ERRORLEVEL% >> "%OUT%\build.txt"
echo === test >> "%OUT%\build.txt"
cargo test --release --target-dir %T% >> "%OUT%\build.txt" 2>&1
echo test EXIT %ERRORLEVEL% >> "%OUT%\build.txt"
echo === clippy >> "%OUT%\build.txt"
cargo clippy --release --all-targets --target-dir %T% -- -D warnings >> "%OUT%\build.txt" 2>&1
echo clippy EXIT %ERRORLEVEL% >> "%OUT%\build.txt"
