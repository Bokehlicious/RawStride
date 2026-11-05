@echo off
rem Builds the CURRENT app (before the M2 RAW pipeline) in release mode and keeps a copy as the
rem M1 reference for performance comparisons: D:\PhotoWorkflowData\builds\m1-reference\
rem Double-click. Log: D:\PhotoWorkflowData\analysis\build-m1-reference.log
setlocal
cd /d "%~dp0.."
set LOG=D:\PhotoWorkflowData\analysis\build-m1-reference.log
set DEST=D:\PhotoWorkflowData\builds\m1-reference
echo === %DATE% %TIME% === > "%LOG%"
echo Building the M1 reference (release, several minutes)...
cargo build --release -p photoworkflow >> "%LOG%" 2>&1
if errorlevel 1 (echo build FAILED - see %LOG% & pause & exit /b 1)
if not exist "%DEST%" mkdir "%DEST%"
copy /y target\release\photoworkflow.exe "%DEST%\photoworkflow.exe" >> "%LOG%"
git rev-parse HEAD > "%DEST%\git-head.txt" 2>> "%LOG%"
git status --short > "%DEST%\git-status.txt" 2>> "%LOG%"
echo Done: %DEST%\photoworkflow.exe
echo DONE >> "%LOG%"
pause
