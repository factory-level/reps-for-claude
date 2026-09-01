# Minimize / restore every *other* app's windows (all workspaces) over X11.
# Run by windows.rs: `python3 -c <this> minimize <our-pid>` prints the ids it
# iconified; `python3 -c <this> restore <our-pid> <ids...>` maps them back.
# Needs the system python3-xlib (Linux Mint ships it).
import sys
from Xlib import X, Xatom, display, protocol

d = display.Display()
root = d.screen().root
A = d.intern_atom
mode, mine = sys.argv[1], int(sys.argv[2])

if mode == "minimize":
    done = []
    prop = root.get_full_property(A("_NET_CLIENT_LIST"), Xatom.WINDOW)
    for wid in (prop.value if prop else []):
        w = d.create_resource_object("window", wid)
        pid = w.get_full_property(A("_NET_WM_PID"), Xatom.CARDINAL)
        if pid and pid.value[0] == mine:
            continue
        state = w.get_full_property(A("WM_STATE"), A("WM_STATE"))
        if state and state.value[0] == X.IconicState:
            continue
        kind = w.get_full_property(A("_NET_WM_WINDOW_TYPE"), Xatom.ATOM)
        if kind and A("_NET_WM_WINDOW_TYPE_NORMAL") not in kind.value:
            continue  # panels, docks, the desktop itself
        ev = protocol.event.ClientMessage(
            window=w, client_type=A("WM_CHANGE_STATE"), data=(32, [X.IconicState, 0, 0, 0, 0])
        )
        root.send_event(ev, event_mask=X.SubstructureRedirectMask | X.SubstructureNotifyMask)
        done.append(wid)
    d.sync()
    print(" ".join(map(str, done)))
elif mode == "activate":
    # Our window by pid + title: map it and ask the WM to activate it as a
    # pager would (source=2) — that bypasses focus-stealing prevention.
    title = sys.argv[3]
    prop = root.get_full_property(A("_NET_CLIENT_LIST"), Xatom.WINDOW)
    for wid in (prop.value if prop else []):
        w = d.create_resource_object("window", wid)
        pid = w.get_full_property(A("_NET_WM_PID"), Xatom.CARDINAL)
        name = w.get_full_property(A("_NET_WM_NAME"), A("UTF8_STRING"))
        if not (pid and pid.value[0] == mine and name and name.value.decode(errors="replace") == title):
            continue
        w.map()
        ev = protocol.event.ClientMessage(
            window=w, client_type=A("_NET_ACTIVE_WINDOW"), data=(32, [2, X.CurrentTime, 0, 0, 0])
        )
        root.send_event(ev, event_mask=X.SubstructureRedirectMask | X.SubstructureNotifyMask)
    d.sync()
else:
    for wid in map(int, sys.argv[3:]):
        d.create_resource_object("window", wid).map()
    d.sync()
