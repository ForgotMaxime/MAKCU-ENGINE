# inspiration — clones tiers (lecture seule, jamais committés)

> Clonés le 2026-11-11 en `--depth 1`. Non couverts par la licence du projet (`MIT OR Apache-2.0`, auteur unique, cf. CHANGELOG 0.5.1). Ne pas copier de code sans vérifier la licence d'origine (dont GPL).

| Dossier | Repo | Intérêt pour MAKCU-ENGINE |
|---|---|---|
| `makcu-py-lib` | `SleepyTotem/makcu-py-lib` (PyPI `makcu`, GPL-3.0) | Référence Python : sync+async, `click_human_like`, `move_smooth/bezier`, `drag`, lock unifié, `batch_execute`, auto-reconnect. |
| `pymakcu` | `NeuralAIM/pymakcu` (Apache-2.0) | Fork sync-only : callbacks fiabilisés, Bezier, jitter. Piste la plus simple à étudier (licence compatible). |
| `makcu` | `K4HVH/makcu` (Rust, MIT) | `Device` sync/async, `BatchBuilder`, `extras`, `profile`, `mock`. Miroir de `makcu_api` ? comparer. |
| `makcu_api` | `terrafirma2021/makcu_api` | Doublon apparent de `K4HVH/makcu` — diff à faire avant de s'en inspirer. |
| `mak-suite` | `terrafirma2021/mak-suite` | Suite unifiée C++/Rust/Python/C# + contrat `MAKXD_PROTOCOL.md`. Source pour la parité inter-langages. |
| `makcu-cpp` | `K4HVH/makcu-cpp` (GPL-3.0) | C++23/C ABI, sub-ms, VID:PID `1A86:55D3`. Patterns hot-path (pas de `sleep`, timeouts courts). |
| `makcu-csharp` | `1claim-gh/makcu-csharp` | API C# (via mak-suite). Comparer la modélisation boutons/stream. |
| `medius` | `K4HVH/medius` | Firmware alternatif + lib Rust (handshake, keepalive, reconnect, `move_rel` i16 avec carry). Idées résilience. |
| `MakcuRelay` | `needitem/MakcuRelay` (MIT, C++/Python) | Relai UDP→série `MOVE:x,y` @ 4 Mbps, 50 ms timeout. Pattern 2e PC / low-latency. |
| `MAKCM` | `terrafirma2021/MAKCM` | Firmware open-source ESP32-S3 dual-MCU, RTOS, 6 Mbps. Comprendre côté device (`km.move` override, logging). |
| `makcu-controller-fw` | `Oso-Cute/makcu-controller-fw` | Fix firmware passthrough manette + GUI Python. Hors scope mouse-only, utile pour `controller_*` (non-goal). |

## Re-cloner / mettre à jour

```powershell
# PowerShell, depuis la racine du projet
Get-ChildItem inspiration -Directory | ForEach-Object { git -C $_.FullName pull --ff-only }
```
