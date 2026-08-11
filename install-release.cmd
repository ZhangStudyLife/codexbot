@echo off
setlocal
chcp 65001 >nul

set "CODEXBOT_ROOT=%~dp0"
if defined CODEXBOT_DATA_DIR (
  set "CODEXBOT_DATA=%CODEXBOT_DATA_DIR%"
) else (
  set "CODEXBOT_DATA=%LOCALAPPDATA%\CodexBot"
)
set "CODEXBOT_BIN=%CODEXBOT_DATA%\bin"
set "CODEXBOT_INSTALLED_EXE=%CODEXBOT_BIN%\codexbot.exe"
set "CODEXBOT_RELEASE_EXE=%CODEXBOT_ROOT%codexbot.exe"

if /I "%CODEXBOT_PARSE_ONLY%"=="1" (
  if not exist "%CODEXBOT_RELEASE_EXE%" exit /b 1
  if not exist "%CODEXBOT_ROOT%plugin\codexbot\.codex-plugin\plugin.json" exit /b 1
  echo [OK] Release 安装包结构完整。
  exit /b 0
)

if not exist "%CODEXBOT_RELEASE_EXE%" (
  echo [ERROR] 安装包中缺少 codexbot.exe，请重新下载并完整解压 Release 压缩包。
  goto :fail
)

echo [1/3] 安装 CodexBot...
if exist "%CODEXBOT_INSTALLED_EXE%" "%CODEXBOT_INSTALLED_EXE%" stop >nul 2>&1
if not exist "%CODEXBOT_BIN%" mkdir "%CODEXBOT_BIN%"
if errorlevel 1 goto :fail
copy /Y "%CODEXBOT_RELEASE_EXE%" "%CODEXBOT_INSTALLED_EXE%" >nul
if errorlevel 1 goto :fail

echo [2/3] 配置 QQ 凭据与个人 Codex 插件...
"%CODEXBOT_INSTALLED_EXE%" setup --repo-root "%CODEXBOT_ROOT%." %*
if errorlevel 1 goto :fail

echo [3/3] 安装完成。
echo 可运行 .\codexbot.cmd doctor --offline 检查状态。
if not defined CODEXBOT_NO_PAUSE pause
exit /b 0

:fail
echo.
echo [ERROR] 安装未完成，请保留上方错误信息并运行 .\codexbot.cmd doctor --offline 排查。
if not defined CODEXBOT_NO_PAUSE pause
exit /b 1
