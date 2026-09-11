"""Reuse the existing local labeler's transport and atomic writes, not its star schema."""

import importlib.util
from pathlib import Path

TOOL_PATH = Path("/Users/qiaopeng/Desktop/测试/QRaw_表情联调_标注工具/expression_labeler.py")
spec = importlib.util.spec_from_file_location("qraw_existing_labeler", TOOL_PATH)
if spec is None or spec.loader is None or not TOOL_PATH.is_file():
    raise RuntimeError(f"既有标注工具不可用：{TOOL_PATH}")
tool = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tool)
content_hash = tool.content_hash
atomic_write_text = tool.atomic_write_text
utc_now = tool.utc_now
