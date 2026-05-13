export const SMART_CULLING_REVIEW_VIEW = 'smart-culling-review';

export const SMART_CULLING_MODES = [
  { value: 'portrait', label: '人像' },
  { value: 'wedding_event', label: '婚礼/活动' },
  { value: 'family_children', label: '儿童/家庭' },
  { value: 'landscape', label: '风光' },
  { value: 'street_documentary', label: '街拍/纪实' },
  { value: 'sports_wildlife', label: '体育/动物/飞鸟' },
  { value: 'product_still', label: '产品/静物' },
  { value: 'architecture', label: '建筑/空间' },
  { value: 'general', label: '通用模式' },
] as const;

export const SMART_CULLING_PRESETS = [
  { value: 'balanced', label: '均衡筛选' },
  { value: 'strict', label: '严格筛选' },
  { value: 'loose', label: '宽松筛选' },
] as const;

export const SMART_CULLING_AESTHETIC_PREFERENCES = [
  { value: 'general', label: '通用模型' },
  { value: 'dark_tone', label: '暗调偏好' },
  { value: 'film', label: '胶片感' },
  { value: 'shallow_depth', label: '浅景深' },
  { value: 'candid_emotion', label: '抓拍情绪' },
] as const;

export const SMART_CULLING_FACE_CHECKS = [
  { value: 'closed_eyes', label: '闭眼' },
  { value: 'blurred_face', label: '糊脸' },
  { value: 'abnormal_expression', label: '表情异常' },
  { value: 'smile', label: '微笑' },
  { value: 'best_group_expression', label: '多人最佳表情' },
  { value: 'looking_camera', label: '主体看镜头' },
] as const;

export const SMART_CULLING_RANGES = [
  { value: 'current_filter', label: '当前筛选结果' },
  { value: 'selected', label: '当前选中图片' },
  { value: 'current_folder', label: '当前文件夹全部图片' },
] as const;
