"""Profiler + macro helpers (MockTransport only, no hardware)."""

import importlib.util
from pathlib import Path

import pytest

from makcu_engine.mock import MockTransport

EXAMPLES = Path(__file__).resolve().parents[1] / "examples"


def _load_example(name: str):
    path = EXAMPLES / f"{name}.py"
    spec = importlib.util.spec_from_file_location(f"examples_{name}", path)
    assert spec is not None and spec.loader is not None
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def test_profiler_off_by_default():
    t = MockTransport()
    assert not t.profiling_enabled()
    t.query("km.version")
    t.set("km.move", 1, 0)
    assert t.get_profile() == {}


def test_profiler_records_query_set():
    t = MockTransport()
    t.enable_profiling()
    assert t.profiling_enabled()
    t.query("km.version")
    t.query("km.version")
    t.set("km.move", 1, 0)
    prof = t.get_profile()
    assert set(prof) == {"km.version", "km.move"}
    assert prof["km.version"]["count"] == 2
    assert prof["km.move"]["count"] == 1
    for entry in prof.values():
        assert entry["total_s"] >= 0.0
        assert entry["avg_ms"] >= 0.0
        assert entry["min_ms"] <= entry["avg_ms"] <= entry["max_ms"] or entry["count"] == 1
        assert entry["min_ms"] >= 0.0


def test_profiler_ignores_ff_and_raw():
    t = MockTransport()
    t.enable_profiling()
    t.fire_and_forget("km.move", 1, 1)
    t.send_raw(b"km.wheel(1)\r")
    assert t.get_profile() == {}
    t.query("km.echo")
    assert set(t.get_profile()) == {"km.echo"}


def test_profiler_reset_and_disable():
    t = MockTransport()
    t.enable_profiling()
    t.query("km.version")
    assert t.get_profile()
    t.reset_profile()
    assert t.get_profile() == {}
    assert t.profiling_enabled()
    t.query("km.version")
    assert t.get_profile()
    t.disable_profiling()
    assert not t.profiling_enabled()


def test_udp_relay_parse():
    udp = _load_example("udp_relay")
    assert udp.parse_datagram("PING") == ("ping", ())
    assert udp.parse_datagram("MOVE:10,-5") == ("move", (10, -5))
    assert udp.parse_datagram("WHEEL:2") == ("wheel", (2,))
    assert udp.parse_datagram("PRESS:LEFT") is not None
    assert udp.parse_datagram("RELEASE:RIGHT") is not None
    assert udp.parse_datagram("CLICK:LEFT") is not None
    assert udp.parse_datagram("MOVE:oops") is None
    assert udp.parse_datagram("km.screen(1,2)") is None
    assert udp.parse_datagram("PRESS:MIDDLE") is None


def test_udp_relay_allows_only_motion():
    udp = _load_example("udp_relay")
    t = MockTransport()
    from makcu_engine import Mouse

    mouse = Mouse(t)
    reply = udp.apply_action(mouse, ("move", (3, 4)))
    assert reply is None
    reply = udp.apply_action(mouse, ("ping", ()))
    assert reply == "PONG"
    sent = " ".join(t.sent)
    assert "km.move" in sent
    # Parser never yields settings/stream commands.
    for bad in ("km.screen(1,2)", "km.buttons(1)", "km.autorelease(5)", "HELLO"):
        assert udp.parse_datagram(bad) is None


def test_macro_helpers():
    macros = _load_example("macros")
    events = [
        {"t_ms": 0, "x": 0, "y": 0},
        {"t_ms": 10, "x": 0, "y": 0},
        {"t_ms": 20, "x": 5, "y": 0},
    ]
    assert macros.dedupe_moves(events) == [
        {"t_ms": 0, "x": 0, "y": 0},
        {"t_ms": 20, "x": 5, "y": 0},
    ]
    assert macros.scale_xy(events[:2], 2.0, 0.5)[1]["x"] == 0
    scaled = macros.scale_xy([{"t_ms": 0, "x": 10, "y": 20}], 0.5, 2.0)
    assert (scaled[0]["x"], scaled[0]["y"]) == (5, 40)
    with pytest.raises(ValueError):
        macros.scale_xy(events, 0.0, 1.0)
    with pytest.raises(ValueError):
        macros.adjust_delays(events, 10.0)
    doubled = macros.adjust_delays(events, 2.0)
    assert [e["t_ms"] for e in doubled] == [0, 20, 40]
    plan = macros.build_plan(events)
    assert plan[0] == (0, 0, 0)
    assert (plan[-1][0], plan[-1][1]) == (5, 0)


def test_macro_plan_relative_only(capsys):
    macros = _load_example("macros")
    events = [{"t_ms": 0, "x": 10, "y": 10}, {"t_ms": 30, "x": 15, "y": 12}]
    plan = macros.build_plan(events)
    assert plan == [(0, 0, 0), (5, 2, 30)]
    assert "move(5,2)" in macros.format_plan(plan)


def test_macro_main_dry_run(tmp_path, capsys):
    import json

    macros = _load_example("macros")
    p = tmp_path / "m.json"
    p.write_text(
        json.dumps(
            {"version": 1, "screenLayout": {"w": 10, "h": 10},
             "events": [{"t_ms": 0, "x": 0, "y": 0}, {"t_ms": 10, "x": 3, "y": 4}]}
        ),
        encoding="utf-8",
    )
    assert macros.main([str(p)]) == 0
    out = capsys.readouterr().out
    assert "move(3,4)" in out


def test_macro_to_rel_deltas_empty_rejected():
    macros = _load_example("macros")
    with pytest.raises(ValueError):
        macros.to_rel_deltas([])


def test_macro_load_rejects_out_of_range_coords(tmp_path):
    import json

    macros = _load_example("macros")
    for bad in (
        {"t_ms": 0, "x": -1, "y": 0},
        {"t_ms": 0, "x": 0, "y": 32768},
        {"t_ms": 0, "x": 40000, "y": 0},
    ):
        p = tmp_path / "bad.json"
        p.write_text(
            json.dumps({"version": 1, "events": [bad]}),
            encoding="utf-8",
        )
        with pytest.raises(ValueError):
            macros.load_macro(p)


def test_macro_scale_xy_renamed_no_shadow():
    macros = _load_example("macros")
    assert callable(macros.scale_xy)
    assert not hasattr(macros, "scale_coordinates")
    assert macros.scale_xy([{"t_ms": 0, "x": 4, "y": 6}], 0.5, 0.5) == [
        {"t_ms": 0, "x": 2, "y": 3}
    ]


def test_macro_replay_still_explicit(tmp_path, capsys):
    import json

    macros = _load_example("macros")
    p = tmp_path / "m.json"
    p.write_text(
        json.dumps(
            {"version": 1, "events": [{"t_ms": 0, "x": 0, "y": 0}, {"t_ms": 5, "x": 1, "y": 1}]}
        ),
        encoding="utf-8",
    )
    # No --replay: dry-run plan, no port needed.
    assert macros.main([str(p)]) == 0
    # --replay without --port fails the same way as before.
    assert macros.main([str(p), "--replay"]) == 2
