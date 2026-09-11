"""Frozen-threshold sensitivity, deliberately not a synthetic accuracy benchmark."""

from collections import Counter

from candidate import finite, metrics, predict


def same_detail(left, right, tolerance):
    if isinstance(left, dict) and isinstance(right, dict):
        return left.keys() == right.keys() and all(same_detail(left[k], right[k], tolerance) for k in left)
    if isinstance(left, list) and isinstance(right, list):
        return len(left) == len(right) and all(same_detail(a, b, tolerance) for a, b in zip(left, right))
    if finite(left) and finite(right):
        return abs(left - right) <= tolerance
    if isinstance(left, str) and isinstance(right, str):
        return left == right
    return left is None and right is None


def summarize(rows, protocol, candidate, predictor=predict, evidence_keys=("detail",)):
    expected = [case["id"] for case in protocol["cases"]]
    if len(rows) != protocol["sample_count"] or len({row["sample_id"] for row in rows}) != len(rows):
        raise ValueError("样本数量不完整或重复")
    by_case = {name: dict(total=0, clear_origin=Counter(), blurry_origin=Counter(),
                         changed=0, detail_changed=0, blurry_promoted=[], guarded_unknown=0) for name in expected}
    originals, annotated, rendered_accepted = [], [], 0
    for row in rows:
        variants = row["variants"]
        if [variant["case_id"] for variant in variants] != expected or row["label"] not in (1, 2):
            raise ValueError("实验矩阵或人工原图标签不一致；不删除困难样本")
        original = variants[0]
        if original["case_id"] != "original" or original["input_source"] != candidate["input_source"]:
            raise ValueError("缺少真正原图对照")
        if not same_detail(original["detail"], row["frozen_detail"], protocol["detail_absolute_tolerance"]):
            raise ValueError("原图复测与冻结观测不同，不能混算敏感性")
        base = predictor(original, candidate)
        originals.append(base)
        rendered_accepted += predictor(dict(original, input_source="renderedRgb8"), candidate) != "uncertain"
        measured = []
        for variant in variants:
            evidence = dict(variant, detail=variant["detail"] or {})
            # Deliberate counterfactual: quantify what happens if pixel provenance
            # is ignored. Never send this overridden input to the app or fit().
            projection = predictor(dict(evidence, input_source=candidate["input_source"]), candidate)
            guarded = predictor(evidence, candidate)
            result = by_case[variant["case_id"]]
            result["total"] += 1
            result["clear_origin" if row["label"] == 1 else "blurry_origin"][projection] += 1
            result["changed"] += projection != base
            result["detail_changed"] += not all(same_detail(original[key], variant[key], protocol["detail_absolute_tolerance"])
                                               for key in evidence_keys)
            result["guarded_unknown"] += guarded == "uncertain"
            if row["label"] == 2 and projection == "clear":
                result["blurry_promoted"].append(row["image_ref"])
            measured.append(dict(variant, numeric_projection=projection, source_guarded_state=guarded))
        annotated.append(dict(row, variants=measured))
    background_changes = sum(by_case[c["id"]]["detail_changed"] for c in protocol["cases"] if c["kind"] == "background")
    dangerous = {name: result["blurry_promoted"] for name, result in by_case.items()
                 if next(c["kind"] for c in protocol["cases"] if c["id"] == name) in {"noise", "noise_sharpen", "unsharpen"}
                 and result["blurry_promoted"]}
    checks = protocol["checks"]
    passed = (background_changes <= checks["background_detail_changes_allowed"]
              and sum(map(len, dangerous.values())) <= checks["blurry_promotions_after_noise_or_sharpen_allowed"]
              and rendered_accepted <= checks["rendered_source_accepted_allowed"])
    return dict(by_case=by_case, baseline=metrics(rows, originals), rows=annotated,
                background_detail_changes=background_changes, noise_sharpen_promotions=dangerous,
                rendered_source_accepted=rendered_accepted, screening_passed=passed,
                synthetic_accuracy_claimed=False, independent_holdout_count=0, release_ready=False)


def markdown(report):
    counts = Counter(row["label"] for row in report["rows"])
    triples = lambda counts: "/".join(str(counts.get(state, 0)) for state in ("clear", "unclear", "uncertain"))
    lines = ["# 清晰度候选 0.1：抗干扰与像素来源检查", "",
        f"固定 {len(report['rows'])} 张已标注原图、16 种条件；未修改阈值、标签、原图或正式评分。",
        f"预设最小抗扰筛查：{'通过' if report['screening_passed'] else '未通过'}。这不是独立准确率验收，候选仍不进入正式评分。", "",
        "## 固定阈值的敏感性结果", "",
        "除原图外，下表都为忽略来源限制后强制套用数值门槛的反事实检查。分栏标签属于原图，不是给变体追加的人工真值；暗化、缩小或模糊后的变化不直接计为准确率。", "",
        f"| 条件 | 原图清晰 {counts[1]} 张 → 清晰/模糊/未知 | 原图模糊 {counts[2]} 张 → 清晰/模糊/未知 | 与原图结论不同 |",
        "| --- | --- | --- | --- |"]
    for case, result in report["by_case"].items():
        lines.append(f"| {case} | {triples(result['clear_origin'])} | {triples(result['blurry_origin'])} | {result['changed']}/{result['total']} |")
    lines += ["", "## 结论与限制", "",
        f"- 只改框外背景：局部细节变化 {report['background_detail_changes']} 次。固定脸框，不代表重新检测后仍能选中同一张脸。",
        f"- 噪声/锐化导致原本模糊照片升为清晰的条件与图片：{report['noise_sharpen_promotions'] or '未出现'}。这类处理不恢复拍摄细节，升档属于候选风险，不能以新增细节分数解释成对焦改善。",
        f"- 将原图观测标为 renderedRgb8 的来源防线检查：接受 {report['rendered_source_accepted']}/{len(report['rows'])}；拒绝不同像素来源是安全行为，不是已兼容预览。",
        "- CPU 缩放不等于应用 GPU 渲染，本轮没有启动 GPU 渲染、重跑人脸检测或模型，也没有 RAW 真值。",
        "- 应用 render_current_state 先解码、应用几何编辑，再 GPU 渲染；runner 的新质量观测使用渲染图。非 RAW 旧锐度指标仅在尺寸一致时回退原图，新局部细节未做此回退。",
        "- 仅尺寸一致不能证明几何一致，例如水平翻转/180° 旋转；这也需要在未来像素来源适配时保护，不能只改 input_source 字符串绕过门禁。",
        "- 本批来源与人工类别完全混杂，且没有独立人物/拍摄组留出。不要要求用户重标这 24 张，也不要拿合成变体补足独立留出的每类 20 张。", "",
        "## 追溯", "", "PROTOCOL.json 是测试前固定的矩阵、条件及代码/输入哈希；candidate.json 是未重新拟合的 0.1 参数副本；observations.json 来自现有 Rust 面部测量；stress_report.json 保留逐图全部输出。所有文件只新建，不覆盖前一版。", ""]
    return "\n".join(lines)
