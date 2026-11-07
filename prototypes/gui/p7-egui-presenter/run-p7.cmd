@echo off
rem ADR 0008 / GUI_REQUIREMENTS P7: image presentation in eframe + egui. Writes results to
rem D:\PhotoWorkflowData\analysis\gui\p7\. Installs nothing; the app is not touched.
rem Part A (texture memory, about 5 min), B (P5 scenarios, about 55 min), C (stress, about 16 min),
rem D (on screen, about 10 min). Do not use the computer meanwhile (part D presses keys).
setlocal
set PART=%1
if "%PART%"=="" set PART=all
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0run-p7.ps1" -Part %PART%
if errorlevel 1 (echo FAILED - see D:\PhotoWorkflowData\analysis\gui\p7\run-p7.log & pause & exit /b 1)
echo Done. Results in D:\PhotoWorkflowData\analysis\gui\p7
pause
