@echo off
rem ADR 0006 step 5: verification of the standalone distribution.
rem  1. release build + standalone folder (dist\PhotoWorkflow)
rem  2. tools\verify-dist.ps1: copies the folder to D:\PhotoWorkflowData\dist-test, checks every
rem     DLL import and the exe for development paths, then runs the app FROM THAT COPY with a
rem     cleaned environment (Windows variables only, no cargo/vcpkg/Visual Studio/ExifTool on PATH):
rem     RAW on, RAW off, and a folder with broken files. The app window opens and closes by itself.
rem Do not use the computer while it runs (about 10 minutes). Results: D:\PhotoWorkflowData\analysis\step5\
setlocal
cd /d "%~dp0.."
set OUT=D:\PhotoWorkflowData\analysis\step5
set LOG=D:\PhotoWorkflowData\analysis\m2-step5.log
echo === %DATE% %TIME% === > "%LOG%"
if not exist "%OUT%" mkdir "%OUT%"
echo [1/2] release build + standalone folder...
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0package-dist.ps1" >> "%LOG%" 2>&1
if errorlevel 1 (echo build FAILED - see %LOG% & pause & exit /b 1)
copy /y dist\PhotoWorkflow\dist-manifest.txt "%OUT%\dist-manifest.txt" >> "%LOG%"
echo [2/2] verification from the copied folder...
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0verify-dist.ps1" >> "%LOG%" 2>&1
if errorlevel 1 (echo verification FAILED - see %LOG% & pause & exit /b 1)
echo Done. Results in %OUT%
echo DONE >> "%LOG%"
pause
