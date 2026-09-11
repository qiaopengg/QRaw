import unittest

import numpy as np
import onnx
import onnxruntime as ort

from export_graph import reshape_flattens


def flatten_model(shape, axis):
    graph = onnx.helper.make_graph(
        [onnx.helper.make_node("Flatten", ["input"], ["output"], axis=axis)], "flatten",
        [onnx.helper.make_tensor_value_info("input", onnx.TensorProto.FLOAT, shape)],
        [onnx.helper.make_tensor_value_info("output", onnx.TensorProto.FLOAT, [None, None])])
    return onnx.helper.make_model(graph, opset_imports=[onnx.helper.make_opsetid("", 17)], ir_version=8)


class ExportGraphTest(unittest.TestCase):
    def test_static_reshape_preserves_flatten_shape_and_element_order(self):
        value = np.arange(120, dtype=np.float32).reshape(2, 3, 4, 5)
        for axis in [0, 1, 2, 4, -1]:
            original = flatten_model(list(value.shape), axis)
            expected = ort.InferenceSession(original.SerializeToString(), providers=["CPUExecutionProvider"]).run(None, {"input": value})
            converted, changes = reshape_flattens(original)
            onnx.checker.check_model(converted, full_check=True)
            actual = ort.InferenceSession(converted.SerializeToString(), providers=["CPUExecutionProvider"]).run(None, {"input": value})
            np.testing.assert_array_equal(actual[0], expected[0])
            self.assertEqual(len(changes), 1)
            self.assertEqual(converted.graph.node[0].op_type, "Reshape")

    def test_unknown_shape_is_rejected_instead_of_guessed(self):
        with self.assertRaisesRegex(ValueError, "static shape"):
            reshape_flattens(flatten_model([None, 3, 7, 7], 1))


if __name__ == "__main__":
    unittest.main()
