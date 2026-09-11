"""Freeze upstream triangular indices for the verified static 7/42px maps."""

from contextlib import contextmanager

import torch


@contextmanager
def fixed_geometry():
    originals = {name: getattr(torch, name) for name in ("triu_indices", "tril_indices")}
    replacements = {}
    for name, function in originals.items():
        offset = 1 if name == "triu_indices" else -1
        # Compute with the official torch operator BEFORE tracing; no mask rewrite.
        cache = {(size, size, offset): function(size, size, offset=offset) for size in (7, 42)}

        def indices(row, col, offset=0, *, dtype=torch.long, layout=torch.strided,
                    device=None, cached=cache):
            key = (int(row), int(col), int(offset))
            if key not in cached or layout != torch.strided:
                raise ValueError("unverified SAMP-Net triangular geometry")
            return cached[key].to(device=device, dtype=dtype)

        replacements[name] = indices
    try:
        for name, function in replacements.items():
            setattr(torch, name, function)
        yield
    finally:
        for name, function in originals.items():
            setattr(torch, name, function)
