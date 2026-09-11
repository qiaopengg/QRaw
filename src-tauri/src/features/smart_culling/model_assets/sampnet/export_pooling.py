"""Exact fixed-shape pooling for the pinned 224px SAMP-Net ONNX export.

PyTorch adaptive bins are [floor(i*N/M), ceil((i+1)*N/M)). The active
network uses equal-width bins, so fixed pooling plus constant gathers is exact.
This context changes only the isolated converter process, never upstream files.
"""

from contextlib import contextmanager

import torch
import torch.nn.functional as functional


def bins(length, count):
    length, count = int(length), int(count)
    starts = [i * length // count for i in range(count)]
    widths = [((i + 1) * length + count - 1) // count - start
              for i, start in enumerate(starts)]
    if len(set(widths)) != 1:
        raise ValueError("this converter requires equal-width adaptive bins")
    return starts, widths[0]


def select_bins(value, axis, starts):
    if starts == list(range(int(value.shape[axis]))):
        return value
    return value.index_select(axis, torch.tensor(starts, device=value.device))


def max_pool1d(value, output_size, return_indices=False):
    if return_indices:
        raise ValueError("pooling indices are not part of SAMP-Net inference")
    count = output_size[0] if isinstance(output_size, (list, tuple)) else output_size
    starts, width = bins(value.shape[-1], count)
    # CoreML accepts rank-4 pooling: the added height is exactly one element.
    result = functional.max_pool2d(value.unsqueeze(2), (1, width), stride=1).squeeze(2)
    return select_bins(result, 2, starts)


def pool2d(value, output_size, operation):
    if not isinstance(output_size, (list, tuple)):
        output_size = (output_size, output_size)
    rows, height = bins(value.shape[-2], output_size[0])
    columns, width = bins(value.shape[-1], output_size[1])
    result = operation(value, (height, width), stride=1)
    return select_bins(select_bins(result, 2, rows), 3, columns)


def max_pool2d(value, output_size, return_indices=False):
    if return_indices:
        raise ValueError("pooling indices are not part of SAMP-Net inference")
    return pool2d(value, output_size, functional.max_pool2d)


def avg_pool2d(value, output_size):
    return pool2d(value, output_size, functional.avg_pool2d)


@contextmanager
def fixed_pooling():
    replacements = {"adaptive_max_pool1d": max_pool1d, "adaptive_max_pool2d": max_pool2d,
                    "adaptive_avg_pool2d": avg_pool2d}
    previous = {name: getattr(functional, name) for name in replacements}
    try:
        for name, replacement in replacements.items():
            setattr(functional, name, replacement)
        yield
    finally:
        for name, original in previous.items():
            setattr(functional, name, original)
