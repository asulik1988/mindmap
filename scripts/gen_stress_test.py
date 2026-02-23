#!/usr/bin/env python3
"""Generate a 1M node FreeMind .mm file with 10K+ depth for stress testing."""

import sys
import os


def main():
    depth_target = 10_000
    siblings_per_level = 99  # + 1 spine node = 100 per level
    # Total: 1 root + depth_target * (1 spine + siblings_per_level) = 1,000,001

    output_path = os.path.join(os.path.dirname(os.path.dirname(__file__)), "stress-test-1m.mm")
    if len(sys.argv) > 1:
        output_path = sys.argv[1]

    node_id = 0
    ts = 1740100000000

    with open(output_path, "w") as f:
        f.write('<map version="1.0.1">\n')
        f.write(
            f'<node TEXT="StressRoot" ID="ID_{node_id}" '
            f'CREATED="{ts}" MODIFIED="{ts}">\n'
        )
        node_id += 1
        ts += 1

        # Open spine nodes (each contains siblings + next spine node)
        for depth in range(1, depth_target + 1):
            pos = ' POSITION="right"' if depth == 1 else ""
            f.write(
                f'<node TEXT="D{depth}" ID="ID_{node_id}" '
                f'CREATED="{ts}" MODIFIED="{ts}"{pos}>\n'
            )
            node_id += 1
            ts += 1

            # Leaf siblings at this level
            for s in range(siblings_per_level):
                f.write(
                    f'<node TEXT="D{depth}S{s}" ID="ID_{node_id}" '
                    f'CREATED="{ts}" MODIFIED="{ts}"/>\n'
                )
                node_id += 1
                ts += 1

            if depth % 1000 == 0:
                print(f"  depth {depth}/{depth_target}...", file=sys.stderr)

        # Close all spine nodes + root
        for _ in range(depth_target):
            f.write("</node>\n")
        f.write("</node>\n")  # root
        f.write("</map>\n")

    size_mb = os.path.getsize(output_path) / (1024 * 1024)
    print(
        f"Generated {node_id} nodes, max depth {depth_target + 1}, "
        f"file size {size_mb:.1f} MB: {output_path}",
        file=sys.stderr,
    )


if __name__ == "__main__":
    main()
