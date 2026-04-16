#!/usr/bin/env python3
"""
Export NIMA aesthetic and technical models to ONNX.
"""
import os
import sys
import subprocess
import urllib.request
import shutil
from pathlib import Path

MODELS_DIR = Path.home() / ".qraw" / "models"
MODELS_DIR.mkdir(parents=True, exist_ok=True)

def export_nima(variant, weights_url, output_name):
    output = MODELS_DIR / output_name
    if output.exists() and output.stat().st_size > 100000:
        print(f"[SKIP] {output_name} already exists ({output.stat().st_size // 1024}KB)")
        return True

    print(f"[EXPORT] {output_name} ...")
    import tensorflow as tf
    print(f"  TensorFlow version: {tf.__version__}")

    weights_path = MODELS_DIR / f"_tmp_{variant}.hdf5"
    if not weights_path.exists():
        print(f"  Downloading weights...")
        urllib.request.urlretrieve(weights_url, str(weights_path))
        print(f"  Downloaded ({weights_path.stat().st_size // 1024}KB)")

    print("  Building model...")
    base = tf.keras.applications.MobileNet(
        input_shape=(224, 224, 3), include_top=False, pooling='avg'
    )
    x = base.output
    x = tf.keras.layers.Dropout(0.75)(x)
    x = tf.keras.layers.Dense(10, activation='softmax')(x)
    model = tf.keras.Model(base.input, x)
    model.load_weights(str(weights_path))

    saved_dir = str(MODELS_DIR / f"_tmp_{variant}_saved")
    if os.path.exists(saved_dir):
        shutil.rmtree(saved_dir)

    print("  Exporting SavedModel...")
    model.export(saved_dir)

    print("  Converting to ONNX...")
    subprocess.check_call([
        sys.executable, "-m", "tf2onnx.convert",
        "--saved-model", saved_dir,
        "--output", str(output),
        "--opset", "13"
    ])

    shutil.rmtree(saved_dir, ignore_errors=True)
    weights_path.unlink(missing_ok=True)

    if output.exists():
        print(f"[OK] {output_name} ({output.stat().st_size // 1024}KB)")
        return True
    else:
        print(f"[FAIL] {output_name} not created")
        return False

if __name__ == "__main__":
    print("NIMA Model Export Script")
    print(f"Output: {MODELS_DIR}")
    print("=" * 50)

    export_nima(
        "aesthetic",
        "https://github.com/idealo/image-quality-assessment/raw/master/models/MobileNet/weights_mobilenet_aesthetic_0.07.hdf5",
        "nima.onnx"
    )
    export_nima(
        "technical",
        "https://github.com/idealo/image-quality-assessment/raw/master/models/MobileNet/weights_mobilenet_technical_0.11.hdf5",
        "nima_technical.onnx"
    )
    print("\nDone!")
