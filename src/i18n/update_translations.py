import json
from pathlib import Path

LOCALES_DIR = Path("./locales")

TRANSLATIONS = {
    "ca": {
        "tethering": {
            "wbTemp": "Temp. WB (K)",
            "wbTempPlaceholder": "p. ex. 5600",
            "expComp": "Comp. Exp.",
            "expCompPlaceholder": "p. ex. 0.0",
            "expMode": "Mode Exp.",
            "metering": "Mode de mesura",
            "generalSettings": "Ajustaments generals",
            "autoApplyPreset": "Aplica preajust automàticament",
            "changePreset": "Canvia",
            "clearPreset": "Neteja el preajust",
            "triggerAutofocus": "Dispara l'enfocament automàtic",
            "toasts": {
                "afFailed": "Error en disparar l'enfocament automàtic",
                "presetApplyFailed": "Error en aplicar el preajust a la captura"
            }
        }
    },
    "de": {
        "tethering": {
            "wbTemp": "Farbtemp. (K)",
            "wbTempPlaceholder": "z. B. 5600",
            "expComp": "Belichtungskorr.",
            "expCompPlaceholder": "z. B. 0.0",
            "expMode": "Belichtungsmodus",
            "metering": "Messmodus",
            "generalSettings": "Allgemeine Einstellungen",
            "autoApplyPreset": "Preset autom. anwenden",
            "changePreset": "Ändern",
            "clearPreset": "Preset entfernen",
            "triggerAutofocus": "Autofokus auslösen",
            "toasts": {
                "afFailed": "Autofokus fehlgeschlagen",
                "presetApplyFailed": "Fehler beim Anwenden des Presets auf die Aufnahme"
            }
        }
    },
    "en": {
        "tethering": {
            "wbTemp": "WB Temp (K)",
            "wbTempPlaceholder": "e.g. 5600",
            "expComp": "Exp. Comp",
            "expCompPlaceholder": "e.g. 0.0",
            "expMode": "Exp. Mode",
            "metering": "Metering Mode",
            "generalSettings": "General Settings",
            "autoApplyPreset": "Auto Apply Preset",
            "changePreset": "Change",
            "clearPreset": "Clear Preset",
            "triggerAutofocus": "Trigger Autofocus",
            "toasts": {
                "afFailed": "Autofocus trigger failed",
                "presetApplyFailed": "Failed to apply preset to captured image"
            }
        }
    },
    "es": {
        "tethering": {
            "wbTemp": "Temp. WB (K)",
            "wbTempPlaceholder": "ej. 5600",
            "expComp": "Comp. Exp.",
            "expCompPlaceholder": "ej. 0.0",
            "expMode": "Modo Exp.",
            "metering": "Modo de medición",
            "generalSettings": "Ajustes generales",
            "autoApplyPreset": "Aplicar preajuste autom.",
            "changePreset": "Cambiar",
            "clearPreset": "Borrar preajuste",
            "triggerAutofocus": "Disparar enfoque automático",
            "toasts": {
                "afFailed": "Error al enfocar automáticamente",
                "presetApplyFailed": "Error al aplicar el preajuste a la captura"
            }
        }
    },
    "fr": {
        "tethering": {
            "wbTemp": "Témperature WB (K)",
            "wbTempPlaceholder": "ex. 5600",
            "expComp": "Comp. Exp.",
            "expCompPlaceholder": "ex. 0.0",
            "expMode": "Mode Exp.",
            "metering": "Mode de mesure",
            "generalSettings": "Paramètres généraux",
            "autoApplyPreset": "Appliquer preset auto.",
            "changePreset": "Changer",
            "clearPreset": "Effacer le preset",
            "triggerAutofocus": "Déclencher l'autofocus",
            "toasts": {
                "afFailed": "Échec du déclenchement de l'autofocus",
                "presetApplyFailed": "Échec de l'application du preset à la photo"
            }
        }
    },
    "it": {
        "tethering": {
            "wbTemp": "Temp. WB (K)",
            "wbTempPlaceholder": "es. 5600",
            "expComp": "Comp. Esp.",
            "expCompPlaceholder": "es. 0.0",
            "expMode": "Modo Esp.",
            "metering": "Misurazione",
            "generalSettings": "Impostazioni generali",
            "autoApplyPreset": "Applica preset autom.",
            "changePreset": "Cambia",
            "clearPreset": "Rimuovi preset",
            "triggerAutofocus": "Attiva autofocus",
            "toasts": {
                "afFailed": "Attivazione autofocus fallita",
                "presetApplyFailed": "Impossibile applicare il preset allo scatto"
            }
        }
    },
    "ja": {
        "tethering": {
            "wbTemp": "色温度 (K)",
            "wbTempPlaceholder": "例: 5600",
            "expComp": "露出補正",
            "expCompPlaceholder": "例: 0.0",
            "expMode": "露出モード",
            "metering": "測光モード",
            "generalSettings": "一般設定",
            "autoApplyPreset": "プリセット自動適用",
            "changePreset": "変更",
            "clearPreset": "プリセットを解除",
            "triggerAutofocus": "AFを実行",
            "toasts": {
                "afFailed": "オートフォーカスの実行に失敗しました",
                "presetApplyFailed": "撮影画像へのプリセット適用に失敗しました"
            }
        }
    },
    "ko": {
        "tethering": {
            "wbTemp": "색온도 (K)",
            "wbTempPlaceholder": "예: 5600",
            "expComp": "노출 보정",
            "expCompPlaceholder": "예: 0.0",
            "expMode": "노출 모드",
            "metering": "측광 모드",
            "generalSettings": "일반 설정",
            "autoApplyPreset": "프리셋 자동 적용",
            "changePreset": "변경",
            "clearPreset": "프리셋 제거",
            "triggerAutofocus": "자동 초점 실행",
            "toasts": {
                "afFailed": "자동 초점 실행에 실패했습니다",
                "presetApplyFailed": "촬영된 이미지에 프리셋 적용 실패"
            }
        }
    },
    "pl": {
        "tethering": {
            "wbTemp": "Temp. barwowa (K)",
            "wbTempPlaceholder": "np. 5600",
            "expComp": "Komp. eksp.",
            "expCompPlaceholder": "np. 0.0",
            "expMode": "Tryb ekspozycji",
            "metering": "Tryb pomiaru",
            "generalSettings": "Ustawienia ogólne",
            "autoApplyPreset": "Automatycznie stosuj preset",
            "changePreset": "Zmień",
            "clearPreset": "Wyczyść preset",
            "triggerAutofocus": "Wyzwól autofocus",
            "toasts": {
                "afFailed": "Błąd wyzwalania autofocusu",
                "presetApplyFailed": "Nie udało się zastosować presetu do zdjęcia"
            }
        }
    },
    "pt": {
        "tethering": {
            "wbTemp": "Temp. WB (K)",
            "wbTempPlaceholder": "ex. 5600",
            "expComp": "Comp. Exp.",
            "expCompPlaceholder": "ex. 0.0",
            "expMode": "Modo Exp.",
            "metering": "Modo de Medição",
            "generalSettings": "Configurações Gerais",
            "autoApplyPreset": "Aplicar Predefinição Auto.",
            "changePreset": "Alterar",
            "clearPreset": "Limpar Predefinição",
            "triggerAutofocus": "Disparar Foco Automático",
            "toasts": {
                "afFailed": "Falha ao acionar foco automático",
                "presetApplyFailed": "Falha ao aplicar predefinição à captura"
            }
        }
    },
    "ru": {
        "tethering": {
            "wbTemp": "Цвет. темп. (K)",
            "wbTempPlaceholder": "напр. 5600",
            "expComp": "Экспокоррекция",
            "expCompPlaceholder": "напр. 0.0",
            "expMode": "Режим эксп.",
            "metering": "Режим замера",
            "generalSettings": "Общие настройки",
            "autoApplyPreset": "Авто-применение пресета",
            "changePreset": "Изменить",
            "clearPreset": "Очистить пресет",
            "triggerAutofocus": "Сфокусироваться",
            "toasts": {
                "afFailed": "Ошибка автофокусировки",
                "presetApplyFailed": "Не удалось применить пресет к снимку"
            }
        }
    },
    "zh-CN": {
        "tethering": {
            "wbTemp": "色温 (K)",
            "wbTempPlaceholder": "例如 5600",
            "expComp": "曝光补偿",
            "expCompPlaceholder": "例如 0.0",
            "expMode": "曝光模式",
            "metering": "测光模式",
            "generalSettings": "常规设置",
            "autoApplyPreset": "自动应用预设",
            "changePreset": "更改",
            "clearPreset": "清除预设",
            "triggerAutofocus": "触发自动对焦",
            "toasts": {
                "afFailed": "触发自动对焦失败",
                "presetApplyFailed": "对捕获的图像应用预设失败"
            }
        }
    },
    "zh-TW": {
        "tethering": {
            "wbTemp": "色溫 (K)",
            "wbTempPlaceholder": "例如 5600",
            "expComp": "曝光補償",
            "expCompPlaceholder": "例如 0.0",
            "expMode": "曝光模式",
            "metering": "測光模式",
            "generalSettings": "一般設定",
            "autoApplyPreset": "自動套用預設集",
            "changePreset": "變更",
            "clearPreset": "清除預設集",
            "triggerAutofocus": "觸發自動對焦",
            "toasts": {
                "afFailed": "觸發自動對焦失敗",
                "presetApplyFailed": "對拍攝的影像套用預設集失敗"
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

    print("Starting translation updates for tethering...")
    for lang, trans in TRANSLATIONS.items():
        file_path = LOCALES_DIR / f"{lang}.json"
        update_json_file(file_path, trans)
    print("Done!")

if __name__ == "__main__":
    main()