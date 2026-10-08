"""Regenerate all standalone documentation PNGs without running the app."""

from build_installation import render as render_build_installation
from context_dataflow import render as render_context_dataflow


if __name__ == "__main__":
    for path in render_context_dataflow() + render_build_installation():
        print(path.name)
