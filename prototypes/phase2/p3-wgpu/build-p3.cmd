@echo off
rem Builds the P3 prototype only (no measurement). Log: D:\PhotoWorkflowData\analysis\phase2\p3\build.txt
setlocal
set OUT=D:\PhotoWorkflowData\analysis\phase2\p3
if not exist "%OUT%" mkdir "%OUT%"
cd /d "%~dp0"
if not exist Cargo.lock copy /y ..\..\..\Cargo.lock Cargo.lock > nul
cargo build --release --target-dir ..\..\..\target\prototypes > "%OUT%\build.txt" 2>&1
echo EXIT %ERRORLEVEL% >> "%OUT%\build.txt"
