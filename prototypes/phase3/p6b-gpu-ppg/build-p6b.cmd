@echo off
rem Builds the P6b prototype only. Log: D:\PhotoWorkflowData\analysis\phase3\p6b\build.txt
setlocal
set OUT=D:\PhotoWorkflowData\analysis\phase3\p6b
if not exist "%OUT%" mkdir "%OUT%"
cd /d "%~dp0"
if not exist Cargo.lock copy /y ..\..\..\Cargo.lock Cargo.lock > nul
cargo build --release --target-dir ..\..\..\target\prototypes > "%OUT%\build.txt" 2>&1
echo EXIT %ERRORLEVEL% >> "%OUT%\build.txt"
