#!/usr/bin/env python3
"""Generate a deep binary FreeMind .mm file: root → 2 children → 2 each → 20 levels deep.
Total nodes: 2^21 - 1 = 2,097,151
"""

import os


def main():
    branching = 2
    depth = 20
    output_path = os.path.join(os.path.dirname(os.path.dirname(__file__)), "stress-test-deep-binary.mm")

    node_id = 0
    ts = 1740100000000

    def next_id():
        nonlocal node_id, ts
        nid = node_id
        node_id += 1
        ts += 1
        return nid, ts - 1

    lines = []

    def write_node(current_depth, max_depth):
        nid, t = next_id()
        text = f"L{current_depth}N{nid}"
        pos = ""
        if current_depth == 1:
            side = "right" if nid % 2 == 0 else "left"
            pos = f' POSITION="{side}"'

        if current_depth >= max_depth:
            lines.append(
                f'<node TEXT="{text}" ID="ID_{nid}" '
                f'CREATED="{t}" MODIFIED="{t}"{pos}/>\n'
            )
        else:
            lines.append(
                f'<node TEXT="{text}" ID="ID_{nid}" '
                f'CREATED="{t}" MODIFIED="{t}"{pos}>\n'
            )
            for _ in range(branching):
                write_node(current_depth + 1, max_depth)
            lines.append("</node>\n")

    # Root
    nid, t = next_id()
    lines.append('<map version="1.0.1">\n')
    lines.append(
        f'<node TEXT="DeepBinaryRoot" ID="ID_{nid}" '
        f'CREATED="{t}" MODIFIED="{t}">\n'
    )

    for _ in range(branching):
        write_node(1, depth)

    lines.append("</node>\n")
    lines.append("</map>\n")

    with open(output_path, "w") as f:
        f.writelines(lines)

    size_mb = os.path.getsize(output_path) / (1024 * 1024)
    print(
        f"Generated {node_id} nodes, branching={branching}, depth={depth}, "
        f"file size {size_mb:.1f} MB: {output_path}"
    )


if __name__ == "__main__":
    main()
