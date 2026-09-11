import unittest

import numpy as np

from preprocess import prepare_inputs


class SampInputTest(unittest.TestCase):
    def test_official_rgb_normalization_and_two_input_shapes(self):
        rgb = np.full((80, 120, 3), [255, 128, 0], dtype=np.uint8)
        inputs = prepare_inputs(rgb)
        self.assertEqual(set(inputs), {"image", "saliency"})
        self.assertEqual(inputs["image"].shape, (1, 3, 224, 224))
        self.assertEqual(inputs["saliency"].shape, (1, 1, 224, 224))
        expected = [(1 - 0.485) / 0.229, (128 / 255 - 0.456) / 0.224, -0.406 / 0.225]
        np.testing.assert_allclose(inputs["image"][0, :, 110, 110], expected, atol=1e-6)
        for value in inputs.values():
            self.assertEqual(value.dtype, np.float32)
            self.assertTrue(value.flags.c_contiguous)
            self.assertTrue(np.isfinite(value).all())

    def test_full_frame_is_resized_without_center_crop(self):
        rgb = np.zeros((40, 200, 3), dtype=np.uint8)
        rgb[:, :20, 0] = 255
        rgb[:, -20:, 2] = 255
        image = prepare_inputs(rgb)["image"]
        self.assertGreater(image[0, 0, 100, 0], 2)
        self.assertGreater(image[0, 2, 100, -1], 2)
        self.assertLess(image[0, 0, 100, 110], -2)

    def test_saliency_uses_full_resolution_frame_not_model_thumbnail(self):
        rgb = np.zeros((312, 468, 3), dtype=np.uint8)
        rgb[50:91, 350:391] = 255
        first = prepare_inputs(rgb)
        from PIL import Image
        thumbnail = np.asarray(Image.fromarray(rgb).resize((224, 224), Image.Resampling.BILINEAR))
        second = prepare_inputs(thumbnail)
        self.assertGreater(np.max(np.abs(first["saliency"] - second["saliency"])), 0.01)

    def test_black_white_and_minimum_size_remain_finite(self):
        for shape in [(6, 6, 3), (6, 19, 3), (120, 80, 3)]:
            for value in [0, 128, 255]:
                inputs = prepare_inputs(np.full(shape, value, dtype=np.uint8))
                saliency = inputs["saliency"]
                self.assertTrue(np.isfinite(saliency).all())
                self.assertGreaterEqual(float(saliency.min()), 0)
                self.assertLessEqual(float(saliency.max()), 1)

    def test_invalid_pixels_are_rejected_without_silent_conversion(self):
        for invalid in [None, np.zeros((8, 8)), np.zeros((8, 8, 3), dtype=np.float32),
                        np.zeros((8, 8, 4), dtype=np.uint8), np.zeros((5, 10, 3), dtype=np.uint8),
                        np.zeros((10, 0, 3), dtype=np.uint8)]:
            with self.assertRaises(ValueError):
                prepare_inputs(invalid)

    def test_readonly_noncontiguous_pixels_are_never_mutated(self):
        rgb = np.arange(120 * 180 * 3, dtype=np.uint32).astype(np.uint8).reshape(120, 180, 3)
        rgb = rgb[:, ::-1]
        before = rgb.copy()
        rgb.flags.writeable = False
        first = prepare_inputs(rgb)
        second = prepare_inputs(rgb)
        np.testing.assert_array_equal(rgb, before)
        for key in first:
            np.testing.assert_array_equal(first[key], second[key])


if __name__ == "__main__":
    unittest.main()
