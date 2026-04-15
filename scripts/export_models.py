#!/usr/bin/env python3
"""
Export ONNX models for QRaw AI Culling V4.
Models are saved to ~/.qraw/models/

Usage: python3 scripts/export_models.py
"""

import os
import sys
import subprocess
import urllib.request
import shutil
from pathlib import Path

MODELS_DIR = Path.home() / ".qraw" / "models"
MODELS_DIR.mkdir(parents=True, exist_ok=True)

def install_deps():
    """Install required Python packages."""
    deps = ["torch", "torchvision", "onnx", "onnxruntime"]
    for dep in deps:
        try:
            __import__(dep.replace("-", "_"))
        except ImportError:
            print(f"Installing {dep}...")
            subprocess.check_call([sys.executable, "-m", "pip", "install", dep, "--quiet"])

def export_yolov8n_face():
    """Export YOLOv8n-face to ONNX."""
    output = MODELS_DIR / "yolov8n-face.onnx"
    if output.exists():
        print(f"[SKIP] {output.name} already exists")
        return

    print("[EXPORT] yolov8n-face.onnx ...")
    try:
        subprocess.check_call([sys.executable, "-m", "pip", "install", "ultralytics", "--quiet"])
        from ultralytics import YOLO
        model = YOLO("yolov8n-face.pt")
        model.export(format="onnx", imgsz=640, simplify=True)
        # Move exported file
        exported = Path("yolov8n-face.onnx")
        if exported.exists():
            shutil.move(str(exported), str(output))
            print(f"[OK] {output}")
        else:
            print("[WARN] yolov8n-face export failed, trying alternative download...")
            download_yolov8n_face_alternative(output)
    except Exception as e:
        print(f"[WARN] YOLOv8 export failed: {e}")
        download_yolov8n_face_alternative(output)

def download_yolov8n_face_alternative(output):
    """Try downloading pre-exported yolov8n-face from common sources."""
    urls = [
        "https://github.com/akanametov/yolov8-face/releases/download/v0.0.0/yolov8n-face.onnx",
    ]
    for url in urls:
        try:
            print(f"  Trying {url} ...")
            urllib.request.urlretrieve(url, str(output))
            if output.exists() and output.stat().st_size > 1000000:
                print(f"[OK] {output}")
                return
        except Exception:
            continue
    print("[FAIL] Could not obtain yolov8n-face.onnx")

def export_nima_aesthetic():
    """Download and convert NIMA aesthetic model to ONNX."""
    output = MODELS_DIR / "nima.onnx"
    if output.exists():
        print(f"[SKIP] {output.name} already exists")
        return

    print("[EXPORT] nima.onnx (aesthetic) ...")
    try:
        subprocess.check_call([sys.executable, "-m", "pip", "install", "tensorflow", "tf2onnx", "--quiet"])
        import tensorflow as tf

        # Download idealo's pre-trained weights
        weights_url = "https://github.com/idealo/image-quality-assessment/raw/master/models/MobileNet/weights_mobilenet_aesthetic_0.07.hdf5"
        weights_path = MODELS_DIR / "nima_aesthetic_weights.hdf5"
        if not weights_path.exists():
            print("  Downloading weights...")
            urllib.request.urlretrieve(weights_url, str(weights_path))

        # Build MobileNet + NIMA head
        base = tf.keras.applications.MobileNet(input_shape=(224, 224, 3), include_top=False, pooling='avg')
        x = base.output
        x = tf.keras.layers.Dropout(0.75)(x)
        x = tf.keras.layers.Dense(10, activation='softmax')(x)
        model = tf.keras.Model(base.input, x)
        model.load_weights(str(weights_path))

        # Save as SavedModel then convert
        saved_model_dir = str(MODELS_DIR / "nima_saved")
        model.save(saved_model_dir)

        subprocess.check_call([
            sys.executable, "-m", "tf2onnx.convert",
            "--saved-model", saved_model_dir,
            "--output", str(output),
            "--opset", "13"
        ])

        # Cleanup
        shutil.rmtree(saved_model_dir, ignore_errors=True)
        weights_path.unlink(missing_ok=True)
        print(f"[OK] {output}")
    except Exception as e:
        print(f"[FAIL] NIMA aesthetic export failed: {e}")

def export_nima_technical():
    """Download and convert NIMA technical model to ONNX."""
    output = MODELS_DIR / "nima_technical.onnx"
    if output.exists():
        print(f"[SKIP] {output.name} already exists")
        return

    print("[EXPORT] nima_technical.onnx ...")
    try:
        subprocess.check_call([sys.executable, "-m", "pip", "install", "tensorflow", "tf2onnx", "--quiet"])
        import tensorflow as tf

        weights_url = "https://github.com/idealo/image-quality-assessment/raw/master/models/MobileNet/weights_mobilenet_technical_0.11.hdf5"
        weights_path = MODELS_DIR / "nima_technical_weights.hdf5"
        if not weights_path.exists():
            print("  Downloading weights...")
            urllib.request.urlretrieve(weights_url, str(weights_path))

        base = tf.keras.applications.MobileNet(input_shape=(224, 224, 3), include_top=False, pooling='avg')
        x = base.output
        x = tf.keras.layers.Dropout(0.75)(x)
        x = tf.keras.layers.Dense(10, activation='softmax')(x)
        model = tf.keras.Model(base.input, x)
        model.load_weights(str(weights_path))

        saved_model_dir = str(MODELS_DIR / "nima_tech_saved")
        model.save(saved_model_dir)

        subprocess.check_call([
            sys.executable, "-m", "tf2onnx.convert",
            "--saved-model", saved_model_dir,
            "--output", str(output),
            "--opset", "13"
        ])

        shutil.rmtree(saved_model_dir, ignore_errors=True)
        weights_path.unlink(missing_ok=True)
        print(f"[OK] {output}")
    except Exception as e:
        print(f"[FAIL] NIMA technical export failed: {e}")

def export_hsemotion():
    """Download and convert HSEmotion model to ONNX."""
    output = MODELS_DIR / "hsemotion.onnx"
    if output.exists():
        print(f"[SKIP] {output.name} already exists")
        return

    print("[EXPORT] hsemotion.onnx ...")
    try:
        subprocess.check_call([sys.executable, "-m", "pip", "install", "hsemotion", "--quiet"])
        import torch
        from hsemotion.facial_emotions import HSEmotionRecognizer

        recognizer = HSEmotionRecognizer(model_name='enet_b0_8_best_afew', device='cpu')
        model = recognizer.model
        model.eval()

        dummy = torch.randn(1, 3, 224, 224)
        torch.onnx.export(
            model, dummy, str(output),
            input_names=["input"],
            output_names=["output"],
            dynamic_axes={"input": {0: "batch"}, "output": {0: "batch"}},
            opset_version=13,
        )
        print(f"[OK] {output}")
    except Exception as e:
        print(f"[FAIL] HSEmotion export failed: {e}")
        print("  You can install manually: pip3 install hsemotion && python3 scripts/export_models.py")

def check_existing():
    """Report which models already exist."""
    models = [
        "yolov8n-face.onnx",
        "face_detection_yunet_2023mar.onnx",
        "emotion-ferplus-8.onnx",
        "2d106det.onnx",
        "nima.onnx",
        "nima_technical.onnx",
        "hsemotion.onnx",
    ]
    print(f"\n{'='*60}")
    print(f"Model directory: {MODELS_DIR}")
    print(f"{'='*60}")
    for m in models:
        path = MODELS_DIR / m
        if path.exists():
            size_mb = path.stat().st_size / 1024 / 1024
            print(f"  ✅ {m:40s} ({size_mb:.1f} MB)")
        else:
            print(f"  ❌ {m:40s} (missing)")
    print()

if __name__ == "__main__":
    print("QRaw AI Culling V4 — Model Export Script")
    print("=" * 60)

    check_existing()

    print("\nStep 1: Installing Python dependencies...")
    install_deps()

    print("\nStep 2: Exporting models...")
    export_yolov8n_face()
    export_hsemotion()
    export_nima_aesthetic()
    export_nima_technical()

    print("\nDone! Final status:")
    check_existing()
