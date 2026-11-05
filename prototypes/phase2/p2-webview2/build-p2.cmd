@echo off
rem Builds the P2 prototype only (no measurement). Log: D:\PhotoWorkflowData\analysis\phase2\p2\build.txt
setlocal
set OUT=D:\PhotoWorkflowData\analysis\phase2\p2
if not exist "%OUT%" mkdir "%OUT%"
cd /d "%~dp0"
if not exist Cargo.lock copy /y ..\..\..\Cargo.lock Cargo.lock > nul
cargo build --release --target-dir ..\..\..\target\prototypes > "%OUT%\build.txt" 2>&1
echo EXIT %ERRORLEVEL% >> "%OUT%\build.txt"
