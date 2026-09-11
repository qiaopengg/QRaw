"""Load the pinned upstream inference definitions without training-path effects."""

import ast
import hashlib
from pathlib import Path
from types import SimpleNamespace

import torch
import torchvision.transforms as transforms

from verify_preprocess import SOURCE_SHA256, official_saliency

SOURCE_HASHES = {
    "cadb_dataset.py": SOURCE_SHA256,
    "config.py": "c98b6b869cf05d4af4505a8e56062b17caf4ac5db4d5e3634245744d574f7380",
    "samp_net.py": "d5733316bfa8e6a31d5bb6f8ac42c0ddaa4601334a0fc2875b15d070a1c3401c",
    "samp_module.py": "9ef05c2769e5b380da24ffd8bc24107cb4ad59442403781bbbac4e97519609f2",
}
WEIGHT_SHA256 = "d3d02f36967734fd6f0d2770380a04b777b5cd800a9ba68d4ccc9656f6fef603"


def checked_tree(directory: Path, name: str):
    data = (directory / name).read_bytes()
    if hashlib.sha256(data).hexdigest() != SOURCE_HASHES[name]:
        raise ValueError(f"official source hash mismatch: {name}")
    return ast.parse(data)


def configuration(directory: Path):
    tree = checked_tree(directory, "config.py")
    definition = next(n for n in tree.body if isinstance(n, ast.ClassDef) and n.name == "Config")
    values = {}
    for node in definition.body:
        if isinstance(node, ast.Assign) and isinstance(node.targets[0], ast.Name):
            try:
                values[node.targets[0].id] = ast.literal_eval(node.value)
            except (ValueError, TypeError):
                # Config's filesystem checks and experiment-name logic are not inference.
                pass
    values["num_attributes"] = len(values["attribute_types"])
    return SimpleNamespace(**values)


def load_model(research: Path):
    directory = research / "upstream/SAMPNet"
    scope = {"__name__": "qraw_sampnet_official"}
    # Execute unchanged imports/definitions; never the upstream training/main entry.
    for name in ["samp_module.py", "samp_net.py"]:
        tree = checked_tree(directory, name)
        body = [node for node in tree.body
                if isinstance(node, (ast.Import, ast.ImportFrom, ast.ClassDef, ast.FunctionDef))
                and not (isinstance(node, ast.ImportFrom) and node.module in ("config", "samp_module"))]
        exec(compile(ast.Module(body=body, type_ignores=[]), str(directory / name), "exec"), scope)
    checkpoint = (research / "samp_net.pth").read_bytes()
    if hashlib.sha256(checkpoint).hexdigest() != WEIGHT_SHA256:
        raise ValueError("official SAMP-Net weight hash mismatch")
    import io
    state = torch.load(io.BytesIO(checkpoint), map_location="cpu", weights_only=True)
    if not state or not all(isinstance(t, torch.Tensor) and torch.isfinite(t).all() for t in state.values()):
        raise ValueError("checkpoint must contain finite tensors only")
    cfg = configuration(directory)
    model = scope["SAMPNet"](cfg, pretrained=False)
    model.load_state_dict(state, strict=True)
    model.eval()
    return model, cfg


def input_reference(research: Path):
    directory = research / "upstream/SAMPNet"
    tree = checked_tree(directory, "cadb_dataset.py")
    constants = {node.targets[0].id: ast.literal_eval(node.value) for node in tree.body
                 if isinstance(node, ast.Assign) and isinstance(node.targets[0], ast.Name)
                 and node.targets[0].id in ("IMAGE_NET_MEAN", "IMAGE_NET_STD")}
    dataset = next(n for n in tree.body if isinstance(n, ast.ClassDef) and n.name == "CADBDataset")
    init = next(n for n in dataset.body if isinstance(n, ast.FunctionDef) and n.name == "__init__")
    transform = next(n.value for n in init.body if isinstance(n, ast.Assign)
                     and isinstance(n.targets[0], ast.Attribute) and n.targets[0].attr == "transformer")
    scope = dict(constants, transforms=transforms, self=configuration(directory))
    transformer = eval(compile(ast.Expression(transform), str(directory / "cadb_dataset.py"), "eval"), scope)
    return transformer, official_saliency(directory / "cadb_dataset.py")
