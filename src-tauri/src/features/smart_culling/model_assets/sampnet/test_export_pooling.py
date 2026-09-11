import unittest

import torch
import torch.nn.functional as functional

from export_pooling import avg_pool2d, bins, fixed_pooling, max_pool1d, max_pool2d


class ExportPoolingTest(unittest.TestCase):
    def test_all_active_max_pool_bins_match_pytorch(self):
        generator = torch.Generator().manual_seed(20260911)
        value = torch.randn(1, 3, 56, 56, generator=generator)
        for size in [42, 54, 28, 1]:
            torch.testing.assert_close(max_pool2d(value, size), functional.adaptive_max_pool2d(value, size),
                                       atol=0, rtol=0)
        vector = torch.randn(1, 1, 2352, generator=generator)
        torch.testing.assert_close(max_pool1d(vector, 784), functional.adaptive_max_pool1d(vector, 784),
                                   atol=0, rtol=0)

    def test_active_average_pool_bins_match_pytorch(self):
        value = torch.randn(1, 5, 7, 7, generator=torch.Generator().manual_seed(42))
        for size in [1, 2, 3]:
            torch.testing.assert_close(avg_pool2d(value, size), functional.adaptive_avg_pool2d(value, size),
                                       atol=1e-6, rtol=1e-6)

    def test_unsupported_geometry_and_indices_fail(self):
        with self.assertRaises(ValueError):
            bins(7, 4)
        with self.assertRaises(ValueError):
            max_pool2d(torch.ones(1, 1, 56, 56), 42, return_indices=True)

    def test_context_restores_pytorch_even_after_failure(self):
        original = functional.adaptive_max_pool2d
        with self.assertRaisesRegex(RuntimeError, "intentional"):
            with fixed_pooling():
                self.assertIs(functional.adaptive_max_pool2d, max_pool2d)
                raise RuntimeError("intentional")
        self.assertIs(functional.adaptive_max_pool2d, original)


if __name__ == "__main__":
    unittest.main()
