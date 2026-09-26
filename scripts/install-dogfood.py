#!/usr/bin/env python3
"""Install a built Linux app for the current user, with login startup and recovery."""
import json, os, shutil, sqlite3, subprocess
from pathlib import Path
repo=Path(__file__).resolve().parents[1]
native=repo/'app/src-tauri'; target=native/'target/release'
for binary in ['app','reps']:
    if not (target/binary).is_file(): raise SystemExit('Build first: cd app/src-tauri && cargo build --release --workspace')
home=Path.home(); install=home/'.local/lib/rfp'; bindir=install/'bin'; resources=install/'lib/RFP'
bindir.mkdir(parents=True,exist_ok=True);resources.mkdir(parents=True,exist_ok=True)
# Stop only this installation before replacing its executable.
subprocess.run(['systemctl','--user','stop','rfp.service'],check=False)
for binary in ['app','reps']: shutil.copy2(target/binary,bindir/binary)
config=json.loads((native/'tauri.conf.json').read_text())
for source,dest in config['bundle']['resources'].items():
    src=native/source;dst=resources/dest
    if src.is_dir():shutil.copytree(src,dst,dirs_exist_ok=True)
    else:dst.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(src,dst)
localbin=home/'.local/bin';localbin.mkdir(parents=True,exist_ok=True)
for name in ['reps','rfp']:
    link=localbin/name
    if link.is_symlink():link.unlink()
    if link.exists():raise SystemExit('Refusing to overwrite existing '+str(link))
    link.symlink_to(bindir/'reps')
data=home/'.local/share/rfp';legacy=home/'.local/share/reps-for-claude'
# Must precede the mkdir: an empty new directory would make the app skip its own migration.
if not data.exists() and legacy.is_dir():legacy.rename(data)
data.mkdir(parents=True,exist_ok=True)
# Keep a consistent backup before the application migrates existing history.
if (data/'reps.sqlite').exists() and not (data/'reps.before-rfp.sqlite').exists():
    with sqlite3.connect(data/'reps.sqlite') as source, sqlite3.connect(data/'reps.before-rfp.sqlite') as backup:source.backup(backup)
if (data/'mode.json').exists() and not (data/'mode.before-rfp.json').exists():shutil.copy2(data/'mode.json',data/'mode.before-rfp.json')
(data/'mode.json').write_text('"workout"')
if (data/'reps.sqlite').exists() and not (data/'reps.before-rfp.sqlite').exists():
    with sqlite3.connect(data/'reps.sqlite') as db:
        db.execute("insert into settings(key,value) values('lock_mode','0') on conflict(key) do update set value='0'")
        db.execute("insert into settings(key,value) values('work_minutes','25') on conflict(key) do update set value='25'")
unit=home/'.config/systemd/user/rfp.service';unit.parent.mkdir(parents=True,exist_ok=True)
# systemd percent expansion and path whitespace must be escaped.
def quote(s):return '"'+str(s).replace('\\','\\\\').replace('"','\\"').replace('%','%%')+'"'
unit.write_text('[Unit]\nDescription=RFP passive workout companion\nAfter=graphical-session.target\nPartOf=graphical-session.target\n\n[Service]\nType=simple\nExecStart='+quote(bindir/'app')+' --background\nRestart=on-failure\nRestartSec=5\nEnvironment='+quote('PATH='+os.environ['PATH'])+'\n\n[Install]\nWantedBy=graphical-session.target\n')
launcher=bindir/'login';launcher.write_text('#!/bin/sh\nsystemctl --user import-environment DISPLAY XAUTHORITY DBUS_SESSION_BUS_ADDRESS XDG_CURRENT_DESKTOP\nexec systemctl --user start rfp.service\n');launcher.chmod(0o755)
startup=home/'.config/autostart/rfp.desktop';startup.parent.mkdir(parents=True,exist_ok=True)
startup.write_text('[Desktop Entry]\nType=Application\nName=RFP: reps for prompts\nExec='+quote(launcher)+'\nTerminal=false\nX-GNOME-Autostart-enabled=true\n')
subprocess.run(['systemctl','--user','daemon-reload'],check=True)
subprocess.run([str(launcher)],check=True)
print('Installed RFP with login startup. Status: systemctl --user status rfp.service')
print('CLI:',link)
