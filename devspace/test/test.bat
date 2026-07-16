@echo off
setlocal EnableExtensions
set "TEST_DIR=%~dp0"
for %%I in ("%TEST_DIR%..") do set "TOOL_ROOT=%%~fI"
set "TOOL_ID=northstar.devspace"
set "TOOL_NAME=North Star DEV Space"
set "TOOL_KIND=rust-cli"

echo ============================================================
echo [SMOKE] %TOOL_ID% smokeTest
echo [INFO] tool=%TOOL_NAME%
echo [INFO] root=%TOOL_ROOT%
echo [INFO] kind=%TOOL_KIND%
echo ============================================================

if not exist "%TOOL_ROOT%\tool.json" (
  echo [FAIL] missing descriptor: %TOOL_ROOT%\tool.json
  set "RC=1"
  goto :done
)
if exist "%TOOL_ROOT%\Cargo.toml" echo [OK] source smokeTest: Cargo.toml present
if exist "%TOOL_ROOT%\src" echo [OK] source smokeTest: src directory present
set "RC=0"
echo [PASS] %TOOL_ID% smokeTest passed

goto :done

:done
echo [RESULT] smokeTest exit=%RC%
echo ============================================================
if not "%NORTHSTAR_TESTALL%"=="1" pause
exit /b %RC%
