import json
from pathlib import Path

LOCALES_DIR = Path("./locales")

TRANSLATIONS = {
    "ca": {
        "settings": {
            "thanks": {
                "list": {
                    "spektrafilm": "Per les LUTs d'emulació de pel·lícula basades en espectres, d'Andrea Volpato, que impulsen les emulacions integrades de RapidRAW. Amb llicència CC BY-SA 4.0.",
                    "libgphoto2": "Per la completa biblioteca de comunicació amb càmeres que impulsa el sistema de tethering i captura remota de RapidRAW."
                }
            }
        },
        "ui": {
            "lut": {
                "filmEmulations": "Emulacions de pel·lícula",
                "customLuts": "LUTs personalitzades"
            }
        }
    },
    "de": {
        "settings": {
            "thanks": {
                "list": {
                    "spektrafilm": "Für die spektralbasierten Filmemulations-LUTs von Andrea Volpato, die RapidRAWs integrierte Filmemulationen ermöglichen. Lizenziert unter CC BY-SA 4.0.",
                    "libgphoto2": "Für die umfassende Kamera-Kommunikationsbibliothek, die RapidRAWs Tethering und Fernauslösung ermöglicht."
                }
            }
        },
        "ui": {
            "lut": {
                "filmEmulations": "Filmemulationen",
                "customLuts": "Eigene LUTs"
            }
        }
    },
    "en": {
        "settings": {
            "thanks": {
                "list": {
                    "spektrafilm": "For the spectrally-based film emulation LUTs by Andrea Volpato, used to power RapidRAW's built-in film emulations. Licensed CC BY-SA 4.0.",
                    "libgphoto2": "For the comprehensive camera communication library powering RapidRAW's tethering and remote capture subsystem."
                }
            }
        },
        "ui": {
            "lut": {
                "filmEmulations": "Film Emulations",
                "customLuts": "Custom LUTs"
            }
        }
    },
    "es": {
        "settings": {
            "thanks": {
                "list": {
                    "spektrafilm": "Por las LUTs de emulación de película basadas en espectros, de Andrea Volpato, que impulsan las emulaciones integradas de RapidRAW. Con licencia CC BY-SA 4.0.",
                    "libgphoto2": "Por la completa biblioteca de comunicación con cámaras que impulsa el sistema de tethering y captura remota de RapidRAW."
                }
            }
        },
        "ui": {
            "lut": {
                "filmEmulations": "Emulaciones de película",
                "customLuts": "LUTs personalizadas"
            }
        }
    },
    "fr": {
        "settings": {
            "thanks": {
                "list": {
                    "spektrafilm": "Pour les LUTs d'émulation de film à base spectrale d'Andrea Volpato, qui alimentent les émulations intégrées de RapidRAW. Sous licence CC BY-SA 4.0.",
                    "libgphoto2": "Pour la bibliothèque complète de communication avec les appareils photo qui alimente le tethering et la capture à distance de RapidRAW."
                }
            }
        },
        "ui": {
            "lut": {
                "filmEmulations": "Émulations de film",
                "customLuts": "LUTs personnalisées"
            }
        }
    },
    "it": {
        "settings": {
            "thanks": {
                "list": {
                    "spektrafilm": "Per le LUT di emulazione pellicola su base spettrale di Andrea Volpato, che alimentano le emulazioni integrate di RapidRAW. Concesse in licenza CC BY-SA 4.0.",
                    "libgphoto2": "Per la completa libreria di comunicazione con le fotocamere che alimenta il tethering e lo scatto remoto di RapidRAW."
                }
            }
        },
        "ui": {
            "lut": {
                "filmEmulations": "Emulazioni pellicola",
                "customLuts": "LUT personalizzate"
            }
        }
    },
    "ja": {
        "settings": {
            "thanks": {
                "list": {
                    "spektrafilm": "RapidRAW の内蔵フィルムエミュレーションを支える、Andrea Volpato 氏によるスペクトルベースのフィルムエミュレーション LUT を提供してくださったことに。ライセンスは CC BY-SA 4.0 です。",
                    "libgphoto2": "RapidRAW のテザリングおよびリモート撮影機能を支える、包括的なカメラ通信ライブラリを提供してくださったことに。"
                }
            }
        },
        "ui": {
            "lut": {
                "filmEmulations": "フィルムエミュレーション",
                "customLuts": "カスタム LUT"
            }
        }
    },
    "ko": {
        "settings": {
            "thanks": {
                "list": {
                    "spektrafilm": "RapidRAW의 내장 필름 에뮬레이션을 구동하는, Andrea Volpato의 스펙트럼 기반 필름 에뮬레이션 LUT를 제공해 주셔서. CC BY-SA 4.0 라이선스입니다.",
                    "libgphoto2": "RapidRAW의 테더링 및 원격 촬영 시스템을 구동하는 포괄적인 카메라 통신 라이브러리를 제공해 주셔서."
                }
            }
        },
        "ui": {
            "lut": {
                "filmEmulations": "필름 에뮬레이션",
                "customLuts": "사용자 LUT"
            }
        }
    },
    "pl": {
        "settings": {
            "thanks": {
                "list": {
                    "spektrafilm": "Za oparte na danych spektralnych LUT-y emulacji filmu autorstwa Andrei Volpato, napędzające wbudowane emulacje filmowe RapidRAW. Licencja CC BY-SA 4.0.",
                    "libgphoto2": "Za kompleksową bibliotekę komunikacji z aparatami, napędzającą tethering i zdalne wyzwalanie w RapidRAW."
                }
            }
        },
        "ui": {
            "lut": {
                "filmEmulations": "Emulacje filmowe",
                "customLuts": "Własne LUT-y"
            }
        }
    },
    "pt": {
        "settings": {
            "thanks": {
                "list": {
                    "spektrafilm": "Pelas LUTs de emulação de filme com base espectral, de Andrea Volpato, que alimentam as emulações integradas do RapidRAW. Licenciadas sob CC BY-SA 4.0.",
                    "libgphoto2": "Pela abrangente biblioteca de comunicação com câmeras que alimenta o tethering e a captura remota do RapidRAW."
                }
            }
        },
        "ui": {
            "lut": {
                "filmEmulations": "Emulações de filme",
                "customLuts": "LUTs personalizadas"
            }
        }
    },
    "ru": {
        "settings": {
            "thanks": {
                "list": {
                    "spektrafilm": "За основанные на спектральных данных LUT эмуляции плёнки от Andrea Volpato, на которых построены встроенные плёночные эмуляции RapidRAW. Лицензия CC BY-SA 4.0.",
                    "libgphoto2": "За обширную библиотеку связи с камерами, на которой построены тезеринг и дистанционная съёмка в RapidRAW."
                }
            }
        },
        "ui": {
            "lut": {
                "filmEmulations": "Эмуляции плёнки",
                "customLuts": "Свои LUT"
            }
        }
    },
    "zh-CN": {
        "settings": {
            "thanks": {
                "list": {
                    "spektrafilm": "感谢 Andrea Volpato 提供基于光谱的胶片模拟 LUT，为 RapidRAW 的内置胶片模拟提供支持。采用 CC BY-SA 4.0 许可。",
                    "libgphoto2": "感谢提供全面的相机通信库，为 RapidRAW 的联机拍摄与远程控制子系统提供支持。"
                }
            }
        },
        "ui": {
            "lut": {
                "filmEmulations": "胶片模拟",
                "customLuts": "自定义 LUT"
            }
        }
    },
    "zh-TW": {
        "settings": {
            "thanks": {
                "list": {
                    "spektrafilm": "感謝 Andrea Volpato 提供基於光譜的底片模擬 LUT，為 RapidRAW 的內建底片模擬提供支援。採用 CC BY-SA 4.0 授權。",
                    "libgphoto2": "感謝提供全面的相機通訊函式庫，為 RapidRAW 的連線拍攝與遠端控制子系統提供支援。"
                }
            }
        },
        "ui": {
            "lut": {
                "filmEmulations": "底片模擬",
                "customLuts": "自訂 LUT"
            }
        }
    }
}

def deep_merge(target: dict, source: dict):
    """Recursively merges source dict into target dict."""
    for key, value in source.items():
        if isinstance(value, dict):
            node = target.setdefault(key, {})
            if isinstance(node, dict):
                deep_merge(node, value)
        else:
            target[key] = value

def sort_dict_recursively(item):
    if isinstance(item, dict):
        return {k: sort_dict_recursively(v) for k, v in sorted(item.items())}
    elif isinstance(item, list):
        return [sort_dict_recursively(x) for x in item]
    return item

def update_json_file(file_path: Path, trans: dict):
    if not file_path.exists():
        print(f"Skipping: {file_path.name} (File not found)")
        return

    try:
        with open(file_path, "r", encoding="utf-8") as f:
            data = json.load(f)
    except json.JSONDecodeError:
        print(f"Error parsing JSON in {file_path.name}. Skipping.")
        return

    deep_merge(data, trans)
    sorted_data = sort_dict_recursively(data)

    with open(file_path, "w", encoding="utf-8") as f:
        json.dump(sorted_data, f, ensure_ascii=False, indent=2)
        f.write("\n")

    print(f"Updated and Sorted: {file_path.name}")

def main():
    if not LOCALES_DIR.exists():
        print(f"Error: Locales directory '{LOCALES_DIR}' does not exist.")
        return

    print("Starting translation updates for LUT film emulations & credits...")
    for lang, trans in TRANSLATIONS.items():
        file_path = LOCALES_DIR / f"{lang}.json"
        update_json_file(file_path, trans)
    print("Done!")

if __name__ == "__main__":
    main()
