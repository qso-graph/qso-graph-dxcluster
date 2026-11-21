<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Choose your nodes when the TUI starts

Keep the cluster nodes and Reverse Beacon Network (RBN) feeds you use in one small
file. When you run `qg-dxc tui` with no `--node` or `--replay` option, it lists the
nodes in that file. Choose up to eight nodes and press Enter to start.

QG DX Cluster doesn't include a list of nodes. You write the file yourself, with
the nodes you're authorized to use.

## Create the file

Save the file as `qso-graph/dxcluster-sources.json` in your configuration folder:

| System | Path |
|---|---|
| Linux | `~/.config/qso-graph/dxcluster-sources.json`, or under `$XDG_CONFIG_HOME` when it's set |
| macOS | `~/.config/qso-graph/dxcluster-sources.json` |
| Windows | `%APPDATA%\qso-graph\dxcluster-sources.json` |

To use a file somewhere else, run `qg-dxc tui --sources <file>`.

This example uses invented callsigns and nonresolving documentation addresses.
Replace them with your own callsign and the nodes you use:

```json
{
  "version": 1,
  "call": "Q1ABC",
  "sources": [
    { "name": "Home node", "node": "dx.example.invalid:7300", "selected": true },
    { "name": "Club node", "node": "club.example.invalid:23", "profile": "arcluster" },
    { "name": "RBN CW", "node": "rbn.example.invalid:7000", "profile": "rbn", "call": "Q1ABC-2" }
  ]
}
```

| Member | Required | Meaning |
|---|---|---|
| `version` | Yes | Always `1`. |
| `call` | See note | The callsign to log in with. |
| `sources` | Yes | Between 1 and 64 nodes. |
| `name` | Yes | What the TUI calls the node: up to 32 letters, digits, spaces, or punctuation, unique in the file. |
| `node` | Yes | `host:port`, or `[address]:port` for an IPv6 address. |
| `profile` | No | `auto` (the default), `dxspider`, `arcluster`, `cccluster`, or `rbn`. |
| `selected` | No | `true` to have the node already chosen when the TUI starts. |
| `call` in a source | No | A different callsign for that node, such as another SSID. |

Note: Every node needs a callsign, either its own `call` or the `call` at the top
of the file.

## Choose and start

1. Run `qg-dxc tui`.
2. Use the Up and Down arrows to move, and press Space to choose a node.
3. Press Enter to start the chosen nodes, between one and eight.

To leave without connecting, press `q` or Escape.

## If the file has a mistake

The TUI doesn't start, and it names the file and the first problem, for example:

```text
qg-dxc: configuration: sources file /home/q1abc/.config/qso-graph/dxcluster-sources.json: sources[1].node: port must be 1 to 65535
```

`sources[1]` is the second node in the list, because counting starts at zero.
Fix the line and run `qg-dxc tui` again. Nothing connects until the whole file is
correct.

The file holds no passwords; QG DX Cluster never sends one. The TUI only reads
the file and never changes it.
