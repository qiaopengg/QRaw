#!/usr/bin/env python3
import os
import urllib.request
import ssl

ssl._create_default_https_context = ssl._create_unverified_context

MODELS_DIR = os.path.expanduser('~/.qraw/models')
os.makedirs(MODELS_DIR, exist_ok=True)

models = [
    ('yolov8n-face.onnx', 'https://github.com/akanametov/yolov8-face/releases/download/v0.0.0/yolov8n-face.onnx'),
    ('face_detection_yunet_2023mar.onnx', 'https://ghp.ci/https://github.com/opencv/opencv_zoo/raw/main/models/face_detection_yunet/face_detection_yunet_2023mar.onnx'),
    ('emotion-ferplus-8.onnx', 'https://ghp.ci/https://github.com/onnx/models/raw/main/validated/vision/body_analysis/emotion_ferplus/model/emotion-ferplus-8.onnx'),
    ('2d106det.onnx', 'https://hf-mirror.com/fofr/comfyui/resolve/main/insightface/models/buffalo_l/2d106det.onnx'),
    ('hsemotion.onnx', 'https://ghp.ci/https://github.com/HSE-asavchenko/hsemotion-onnx/raw/main/hsemotion_onnx/models/enet_b0_8_best_afew.onnx'),
    ('nima.onnx', 'https://hf-mirror.com/chavinlo/nima-aesthetic/resolve/main/nima_aesthetic.onnx')
]

for name, url in models:
    dest = os.path.join(MODELS_DIR, name)
    if os.path.exists(dest) and os.path.getsize(dest) > 10000:
        print(f'Already exists: {name}')
        continue
    print(f'Downloading {name} from {url}...')
    try:
        urllib.request.urlretrieve(url, dest)
        print(f'Success: {name}')
    except Exception as e:
        print(f'Failed: {name} - {e}')
