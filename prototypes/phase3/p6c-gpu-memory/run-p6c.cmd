@echo off
rem ADR 0008 P6c: GPU PPG memory budget and texture-cache validation. Writes results to
rem D:\PhotoWorkflowData\analysis\phase3\p6c\. Installs nothing; the app is not touched.
rem Part A (memory, about 40 min), B (P5 comparison, about 60 min), C (on screen, about 6 min).
rem Do not use the computer meanwhile (part C presses keys).
setlocal
set PART=%1
if "%PART%"=="" set PART=all
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0run-p6c.ps1" -Part %PART%
if errorlevel 1 (echo FAILED - see D:\PhotoWorkflowData\analysis\phase3\p6c\run-p6c.log & pause & exit /b 1)
echo Done. Results in D:\PhotoWorkflowData\analysis\phase3\p6c
pause
