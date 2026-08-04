# Smart Culling model notices

These model files are distributed with QRaw for fully offline Smart Culling.

## SFace face recognizer

- File: `face_recognition_sface_2021dec_coreml.onnx`
- Source: https://github.com/opencv/opencv_zoo/tree/main/models/face_recognition_sface
- Original SHA-256: `0ba9fbfa01b5270c96627c4ef784da859931e02f04419c829e83484087c34e79`
- Distributed SHA-256: `3e4a66d8a95745ce8b972e78d1330918db04bdb8ef4a81d02088c50aa8d55a15`
- License: Apache License 2.0 (https://www.apache.org/licenses/LICENSE-2.0)

Copyright (C) 2021, Shenzhen Institute of Artificial Intelligence and Robotics
for Society, all rights reserved. Third-party copyrights are property of their
respective owners.

QRaw modifies the ONNX graph without changing the model weights' mathematical
result: adjacent BatchNorm parameters are folded and the final Gemm is
represented as an equivalent 7x7 Conv whose `1x128x1x1` output contains the
same 128 values. This avoids a standalone Flatten operation unsupported by
the packaged ONNX Runtime/Core ML MLProgram path. The equivalent Conv
explicitly declares zero padding, unit stride, unit dilation, and one group.
The original and distributed hashes above identify both forms. The model is
distributed under the Apache License, Version 2.0, without warranties or
conditions beyond those stated by that license.

## YuNet face detector

- File: `face_detection_yunet_2023mar.onnx`
- Source: https://github.com/opencv/opencv_zoo/tree/main/models/face_detection_yunet
- SHA-256: `8f2383e4dd3cfbb4553ea8718107fc0423210dc964f9f4280604804ed2552fa4`

MIT License

Copyright (c) 2020 Shiqi Yu <shiqi.yu@gmail.com>

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## OCEC open/closed-eye classifier

- File: `ocec_l.onnx`
- Source: https://github.com/PINTO0309/OCEC
- SHA-256: `de9b8031f8b521a862d8cff55ba88c2fccab6ac96484ba53154dd12c53c7c7f9`

MIT License

Copyright (c) 2025 Katsuya Hyodo

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

## FER+ expression classifier

- File: `emotion_ferplus_8.onnx`
- Source: https://github.com/onnx/models (ONNX Model Zoo, `emotion-ferplus-8`)
- Upstream training source: https://github.com/ebarsoum/FERPlus
- SHA-256: `a2a2ba6a335a3b29c21acb6272f962bd3d47f84952aaffa03b60986e04efa61c`
- License: MIT (as stated on the ONNX Model Zoo model card)
- Paper: "Training Deep Networks for Facial Expression Recognition with
  Crowd-Sourced Label Distribution", arXiv:1608.01041

Note on how QRaw uses this model: only the certainty (peakedness) of the output
distribution is consumed, to judge whether a captured instant is technically
usable. The individual emotion classes are never interpreted, surfaced, or used
to influence a photo's rating.

MIT License

Copyright (c) Microsoft Corporation

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
