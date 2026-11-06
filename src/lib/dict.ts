// Dictionaries and the pure translate function (no runes, so it is unit-testable under bun).

export type Dict = Record<string, string>;

export const en: Dict = {
  'tab.today': 'Today',
  'tab.place': 'Place',
  'tab.look': 'Look',
  'tab.schedule': 'Schedule',
  'tab.history': 'History',

  'today.empty.title': 'Pick a place',
  'today.empty.text': 'Choose a country, a city or even a single temple — a beautiful photo of it becomes your wallpaper.',
  'today.empty.cta': 'Choose a place',
  'today.first.title': 'Ready when you are',
  'today.first.text': 'Get the first wallpaper of {place}.',
  'today.first.cta': 'Get a wallpaper',
  'today.like': 'Like',
  'today.skip': 'Not for me',
  'today.change': 'Change now',
  'today.details': 'Details',
  'today.loading': 'Looking for a beautiful photo…',

  'details.title': 'About this photo',
  'details.author': 'Author',
  'details.license': 'License',
  'details.source': 'Source page',
  'details.place': 'Place',
  'details.close': 'Close',
  'details.apply': 'Set as wallpaper',
  'details.applying': 'Setting…',

  'place.title': 'Place',
  'place.search': 'Search: Japan, Kyoto, Mount Fuji, Shibuya…',
  'place.current': 'Current place',
  'place.none': 'No place chosen yet',
  'place.radius': 'Search radius',
  'place.radius.auto': 'Automatic ({km} km)',
  'place.radius.hint': 'Photos geotagged within this distance of the place.',
  'place.wide': 'Large area: photos are sampled from notable spots inside it.',
  'place.notfound': 'Nothing found',
  'place.searching': 'Searching…',
  'place.kind.country': 'Country',
  'place.kind.region': 'Region',
  'place.kind.city': 'City',
  'place.kind.district': 'District',
  'place.kind.landmark': 'Landmark',
  'place.kind.natural': 'Nature',
  'place.kind.other': 'Place',

  'look.title': 'Look',
  'look.tone': 'Photo tone',
  'look.tone.any': 'Any',
  'look.tone.dark': 'Dark',
  'look.tone.light': 'Light',
  'look.tone.hint': 'Only photos of this tone are ever shown.',
  'look.follow': 'Follow the system theme',
  'look.follow.hint': 'Dark photos in dark mode, light photos in light mode.',
  'look.palette': 'Palette of the current wallpaper',
  'look.caption': 'Caption on the wallpaper',
  'look.caption.show': 'Show place and credit',
  'look.caption.show.hint': 'Photo credit is a licence requirement; it stays visible in the app either way.',
  'look.caption.position': 'Position',
  'look.caption.position.hint': 'Move it where home screen icons do not cover it.',
  'look.caption.top': 'Top',
  'look.caption.bottom': 'Bottom',
  'look.align': 'Alignment',
  'look.align.left': 'Left',
  'look.align.center': 'Centre',
  'look.align.right': 'Right',
  'look.language': 'Language',
  'look.lang.auto': 'Automatic',

  'schedule.title': 'Schedule',
  'schedule.time': 'Change wallpaper at',
  'schedule.wifi': 'Download on Wi-Fi only',
  'schedule.wifi.hint': 'The wallpaper is prepared in advance, so the daily change itself works offline.',
  'schedule.cache': 'Cache size',
  'schedule.cache.used': '{used} of {budget} used · {ready} ready to show',
  'schedule.clear': 'Clear cache',
  'schedule.forget': 'Forget everything',
  'schedule.forget.confirm': 'Tap again to erase history, likes and learned taste',
  'schedule.forget.hint': 'Everything stays on this device. This wipes it.',
  'schedule.android': 'Applied by the Android background job.',

  'history.title': 'History',
  'history.empty': 'Wallpapers you have seen will appear here.',
  'history.liked': 'Liked',
  'history.skipped': 'Skipped',

  'error.network': 'No connection to Wikimedia. Check your internet and try again.',
  'error.not_found': 'No suitable photo found for this place and filters. Try a larger place or another tone.',
  'error.config': 'Please choose a place first.',
  'error.parse': 'Unexpected answer from the photo service. Try again later.',
  'error.image': 'A photo could not be processed. Try again.',
  'error.storage': 'Could not read or write local data.',
  'error.internal': 'Something went wrong.',
  'error.dismiss': 'Dismiss',
};

export const ru: Dict = {
  'tab.today': 'Сегодня',
  'tab.place': 'Место',
  'tab.look': 'Вид',
  'tab.schedule': 'Расписание',
  'tab.history': 'История',

  'today.empty.title': 'Выберите место',
  'today.empty.text': 'Страна, город или отдельный храм — красивая фотография этого места станет вашими обоями.',
  'today.empty.cta': 'Выбрать место',
  'today.first.title': 'Всё готово',
  'today.first.text': 'Подберём первые обои: {place}.',
  'today.first.cta': 'Подобрать обои',
  'today.like': 'Нравится',
  'today.skip': 'Не моё',
  'today.change': 'Сменить сейчас',
  'today.details': 'Подробнее',
  'today.loading': 'Ищем красивое фото…',

  'details.title': 'Об этом фото',
  'details.author': 'Автор',
  'details.license': 'Лицензия',
  'details.source': 'Страница источника',
  'details.place': 'Место',
  'details.close': 'Закрыть',
  'details.apply': 'Поставить на обои',
  'details.applying': 'Ставим…',

  'place.title': 'Место',
  'place.search': 'Поиск: Япония, Киото, гора Фудзи, Сибуя…',
  'place.current': 'Текущее место',
  'place.none': 'Место ещё не выбрано',
  'place.radius': 'Радиус поиска',
  'place.radius.auto': 'Автоматически ({km} км)',
  'place.radius.hint': 'Фото с геометкой не дальше этого расстояния от места.',
  'place.wide': 'Большая территория: фото берутся из заметных мест внутри неё.',
  'place.notfound': 'Ничего не найдено',
  'place.searching': 'Ищем…',
  'place.kind.country': 'Страна',
  'place.kind.region': 'Регион',
  'place.kind.city': 'Город',
  'place.kind.district': 'Район',
  'place.kind.landmark': 'Достопримечательность',
  'place.kind.natural': 'Природа',
  'place.kind.other': 'Место',

  'look.title': 'Вид',
  'look.tone': 'Тон фото',
  'look.tone.any': 'Любой',
  'look.tone.dark': 'Тёмные',
  'look.tone.light': 'Светлые',
  'look.tone.hint': 'Будут показываться только фото этого тона.',
  'look.follow': 'Следовать системной теме',
  'look.follow.hint': 'Тёмные фото в тёмной теме, светлые — в светлой.',
  'look.palette': 'Палитра текущих обоев',
  'look.caption': 'Подпись на обоях',
  'look.caption.show': 'Показывать место и автора',
  'look.caption.show.hint': 'Указание автора — требование лицензии; в приложении оно остаётся видимым в любом случае.',
  'look.caption.position': 'Положение',
  'look.caption.position.hint': 'Сдвиньте туда, где её не закрывают ярлыки рабочего стола.',
  'look.caption.top': 'Сверху',
  'look.caption.bottom': 'Снизу',
  'look.align': 'Выравнивание',
  'look.align.left': 'Слева',
  'look.align.center': 'По центру',
  'look.align.right': 'Справа',
  'look.language': 'Язык',
  'look.lang.auto': 'Автоматически',

  'schedule.title': 'Расписание',
  'schedule.time': 'Менять обои в',
  'schedule.wifi': 'Загружать только по Wi-Fi',
  'schedule.wifi.hint': 'Обои готовятся заранее, поэтому сама ежедневная смена работает без сети.',
  'schedule.cache': 'Размер кэша',
  'schedule.cache.used': 'Занято {used} из {budget} · готово к показу: {ready}',
  'schedule.clear': 'Очистить кэш',
  'schedule.forget': 'Забыть всё',
  'schedule.forget.confirm': 'Нажмите ещё раз: сотрутся история, лайки и «вкус»',
  'schedule.forget.hint': 'Всё хранится только на этом устройстве. Это сотрёт всё.',
  'schedule.android': 'Применяется фоновой задачей Android.',

  'history.title': 'История',
  'history.empty': 'Здесь появятся обои, которые вы уже видели.',
  'history.liked': 'Понравилось',
  'history.skipped': 'Пропущено',

  'error.network': 'Нет связи с Wikimedia. Проверьте интернет и попробуйте снова.',
  'error.not_found': 'Для этого места и фильтров не нашлось подходящих фото. Попробуйте место покрупнее или другой тон.',
  'error.config': 'Сначала выберите место.',
  'error.parse': 'Неожиданный ответ сервиса фотографий. Попробуйте позже.',
  'error.image': 'Не удалось обработать фото. Попробуйте снова.',
  'error.storage': 'Не удалось прочитать или записать данные.',
  'error.internal': 'Что-то пошло не так.',
  'error.dismiss': 'Скрыть',
};


export type Language = 'en' | 'ru';

export const dictionaries: Record<Language, Dict> = { en, ru };

export function detectLang(navigatorLanguage: string): Language {
  return navigatorLanguage.toLowerCase().startsWith('ru') ? 'ru' : 'en';
}

export function translate(lang: Language, key: string, params: Record<string, string | number> = {}): string {
  const raw = dictionaries[lang][key] ?? en[key] ?? key;
  return raw.replace(/\{(\w+)\}/g, (_, k: string) => String(params[k] ?? `{${k}}`));
}

/** Keys that exist in one dictionary but not the other (checked by a unit test). */
export function missingKeys(): { en: string[]; ru: string[] } {
  const e = Object.keys(en);
  const r = Object.keys(ru);
  return { en: r.filter((k) => !(k in en)), ru: e.filter((k) => !(k in ru)) };
}
