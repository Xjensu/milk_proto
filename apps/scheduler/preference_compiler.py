"""Conservative Russian teacher-wish parser for the first local prototype."""

import re


WEEKDAYS = {
    "понедельник": "monday", "понедельника": "monday", "понедельнике": "monday",
    "понедельникам": "monday", "понедельниках": "monday",
    "вторник": "tuesday", "вторника": "tuesday", "вторнике": "tuesday",
    "вторникам": "tuesday", "вторниках": "tuesday",
    "среда": "wednesday", "среды": "wednesday", "среде": "wednesday", "средам": "wednesday",
    "среду": "wednesday", "средой": "wednesday", "средами": "wednesday", "средах": "wednesday",
    "четверг": "thursday", "четверга": "thursday", "четверге": "thursday",
    "четвергам": "thursday", "четвергах": "thursday",
    "пятница": "friday", "пятницы": "friday", "пятнице": "friday", "пятницу": "friday",
    "пятницам": "friday", "пятницах": "friday",
    "суббота": "saturday", "субботы": "saturday", "субботе": "saturday",
    "субботу": "saturday", "субботой": "saturday", "субботам": "saturday", "субботах": "saturday",
}
WEEKDAY_ORDER = ("monday", "tuesday", "wednesday", "thursday", "friday", "saturday")
WEEKDAY_PATTERN = "(?:" + "|".join(sorted(WEEKDAYS, key=len, reverse=True)) + ")"
ROOM_CAPABILITY_CONTEXT = re.compile(r"\b(?:аудитори\w*|кабинет\w*|лаборатор\w*)\b")
SOFTWARE_CAPABILITY_CONTEXT = re.compile(
    r"\b(?:программ\w*|компас(?:-?3d)?|софт\w*|установлен\w*|обеспечени\w*)\b"
)
SUBJECT_STOP_WORDS = {
    "для", "или", "при", "через", "между", "в", "из", "на", "по", "к", "и", "с",
    "от", "под", "над", "без", "до", "после",
}
ORDINALS = {
    1: r"перв(?:ый|ого|ому|ом|ая|ой|ую|ое|ые|ых|ым|ыми)",
    2: r"втор(?:ой|ого|ому|ом|ая|ую|ое|ые|ых|ым|ыми)",
    3: r"трет(?:ий|ьего|ьему|ьем|ья|ьей|ью|ье|ьи|ьих|ьим|ьими)",
    4: r"четверт(?:ый|ого|ому|ом|ая|ой|ую|ое|ые|ых|ым|ыми)",
    5: r"пят(?:ый|ого|ому|ом|ая|ой|ую|ое|ые|ых|ым|ыми)",
    6: r"шест(?:ой|ого|ому|ом|ая|ой|ую|ое|ые|ых|ым|ыми)",
    7: r"седьм(?:ой|ого|ому|ом|ая|ой|ую|ое|ые|ых|ым|ыми)",
    8: r"восьм(?:ой|ого|ому|ом|ая|ой|ую|ое|ые|ых|ым|ыми)",
    9: r"девят(?:ый|ого|ому|ом|ая|ой|ую|ое|ые|ых|ым|ыми)",
}
PAIR_NOUN = r"пар(?:а|ы|е|у|ой|ою|ам|ами|ах)?"
CONSECUTIVE_LESSON_NOUN = rf"(?:{PAIR_NOUN}|заняти\w*|урок\w*|лекци\w*|практик\w*|лабораторн\w*|семинар\w*)"
PAIR_COUNT_WORD = (
    r"ноль|нуль|один|одна|одно|одну|одной|одного|одним|два|две|двух|"
    r"три|тр[её]х|четыре|четыр[её]х|пять|пяти|шесть|шести|семь|семи|"
    r"восемь|восьми|девять|девяти|десять|десяти|одиннадцать|одиннадцати|"
    r"двенадцать|двенадцати|тринадцать|тринадцати|четырнадцать|четырнадцати|"
    r"пятнадцать|пятнадцати|шестнадцать|шестнадцати|семнадцать|семнадцати|"
    r"восемнадцать|восемнадцати|девятнадцать|девятнадцати|двадцать|двадцати|"
    r"тридцать|тридцати|сорок|сорока|пятьдесят|пятидесяти|сто|ста|"
    r"полтора|полторы|полутора|"
    r"тысяча|тысячи|тысячу|тысяч"
)
ORDINAL_PATTERN = "(?:" + "|".join(ORDINALS.values()) + ")"
UNSUPPORTED_CALENDAR_SCOPE = re.compile(
    rf"\b{ORDINAL_PATTERN}\s+{WEEKDAY_PATTERN}\b.{{0,32}}\b(?:месяц|семестр)\w*\b|"
    rf"\b{WEEKDAY_PATTERN}\b.{{0,32}}\b{ORDINAL_PATTERN}\s+недел\w*\b"
)
UNSUPPORTED_WEEK_CADENCE = re.compile(
    rf"\bраз\s+в\s+(?:{PAIR_COUNT_WORD})\s+недел\w*\b|"
    rf"\bчерез\s+(?:(?:{PAIR_COUNT_WORD})\s+)?недел\w*\b|"
    rf"\bкажд\w*\s+(?:втор\w*|треть\w*|четверт\w*|пят\w*|шест\w*)\s+недел\w*\b"
)
RELATIVE_PERIOD_SCOPE = re.compile(
    rf"\b(?:до|после|с(?:о)?)\s+(?:[1-9](?:[-‑–]?(?:я|й|ую|ой))?|{ORDINAL_PATTERN})\s*{PAIR_NOUN}\b|"
    rf"\bначиная\s+(?:с|со|после)\s+(?:[1-9](?:[-‑–]?(?:я|й|ую|ой))?|{ORDINAL_PATTERN})\s*{PAIR_NOUN}\b|"
    rf"\bне\s+(?:раньше|позже|ранее|позднее)\b[^.!?;\n]{{0,32}}?"
    rf"\b(?:[1-9](?:[-‑–]?(?:я|й|ую|ой))?|{ORDINAL_PATTERN})\s*{PAIR_NOUN}\b"
)
PAIR_COUNT_PREFIX = (
    rf"\d+(?:\s*[-‑–—]\s*\d+)?(?:[-‑–—][а-яё]+)?|"
    rf"(?:{PAIR_COUNT_WORD})(?:\s*[-‑–—]\s*(?:{PAIR_COUNT_WORD}))?"
    rf"(?:\s+(?:{PAIR_COUNT_WORD}))?"
)
NON_LESSON_COUNT_UNIT = (
    r"(?:дн\w*|недел\w*|сут\w*|месяц\w*|семестр\w*|год\w*|курс\w*|"
    r"смен\w*|час\w*|период\w*|раз(?:а|ов|у|ом|е|ы)?)"
)
TWO_CONSECUTIVE_COUNTS = {"два", "две", "двух"}
CONSECUTIVE_COUNT_QUALIFIER = re.compile(
    rf"\b(?:(?:больше|меньше|более|менее|свыше|около|примерно|порядка|минимум|максимум)\s+(?:чем\s+)?|"
    r"не\s+(?:менее|более|меньше|больше)\s+|как\s+(?:минимум|максимум)\s+)"
)
UNAVAILABLE_INTENT = re.compile(
    r"\bне\s+(?:могу|смогу|буду|получается|подходит|подойдут|удобно|удобен|удобна|"
    r"хочу|хотел(?:а|ось)?|предпочитаю|предпочтительн\w*|в|на|"
    r"став(?:ьте|ить)|назнач(?:ьте|ать)|планируйте|планировать|провод\w*|провож\w*|"
    r"вед\w*|вест\w*|работ\w*|занима\w*|препода\w*|чита\w*)\b|"
    r"\bнеудоб\w*|\bнежелательн\w*|\bне\s+желательн\w*|\bнельзя\b|\bневозмож\w*|\bнедоступ\w*|\bзанят(?:а|о|ы)?\b|"
    r"\bизбег\w*|\bне\s+допуска\w*"
)
NEGATED_WISH_INTENT = re.compile(
    r"\b(?:хочу|хотел(?:а|ось)?|предпочитаю|лучше|желательно)\s+(?:не|избег\w*)\b"
)
NEGATED_AVOIDANCE_INTENT = re.compile(r"\bне(?:\s+\w+){0,3}\s+избег\w*\b")
DOUBLE_NEGATION = re.compile(
    r"\bне\s+(?:неудоб\w*|нежелательн\w*|невозмож\w*|недоступ\w*|нельзя)\b|"
    r"\bне\s+(?:могу|смогу|буду|хочу|хотел(?:а|ось)?|предпочитаю|должен|должна|должны|"
    r"нужно|надо|следует|стоит)\b(?:\s+\w+){0,4}\s+не\s+"
    r"[а-яё]+(?:ть|ти|чь|ться|тись|чься)\b"
)
CONDITIONAL_SCOPE = re.compile(
    r"\b(?:если|кроме|за\s+исключением|при\s+условии|только\s+когда)\b"
)
SUBJECT_REFERENCE = re.compile(r"\bпо\s+(?!расписанию\b|причине\b|поводу\b)[а-яё]{4,}\b")
AVAILABLE_INTENT = re.compile(
    r"\b(?:хочу|предпочитаю|предпочтительн\w*|лучше|удобно|удобен|удобна|"
    r"желательн\w*|подходит|могу|свободен|свободна|назначьте|поставьте|запланируйте)\b"
)


def compile_wishes(data):
    """Return proposed typed rules and review issues; never activate hard rules."""
    requirements = data.get("requirements", [])
    profile = data.get("profile_constraints", [])
    proposals, findings = [], []
    for wish in data.get("teacher_wishes", []):
        # Russian writers vary between ё and е. Treat them alike for parsing
        # and subject lookup while preserving the original source text in storage.
        text = wish["text"].casefold().replace("ё", "е")
        source = wish["key"]
        teacher = wish["teacher"]
        scoped_requirement = str(wish.get("requirement") or "").strip() or None
        teacher_requirements = [row for row in requirements if teacher in _teachers(row)]
        if scoped_requirement and not any(row["key"] == scoped_requirement for row in teacher_requirements):
            findings.append(_finding("UNSUPPORTED_PREFERENCE", "review", [source],
                                     "Выбранная запись нагрузки больше не назначена этому преподавателю; проверьте пожелание."))
            continue

        if CONDITIONAL_SCOPE.search(text):
            findings.append(_finding(
                "UNSUPPORTED_PREFERENCE", "review", [source],
                "Пожелание содержит условие или исключение, которое пока нельзя учесть; требуется проверка администратора."
            ))
            continue

        if (UNSUPPORTED_CALENDAR_SCOPE.search(text) or UNSUPPORTED_WEEK_CADENCE.search(text)
                or RELATIVE_PERIOD_SCOPE.search(text)):
            findings.append(_finding(
                "UNSUPPORTED_PREFERENCE", "review", [source],
                "Пожелание зависит от календарного повторения или интервала относительно пары, который пока нельзя учесть."
            ))
            continue

        if ROOM_CAPABILITY_CONTEXT.search(text) and SOFTWARE_CAPABILITY_CONTEXT.search(text):
            # Preserve the teacher's request for review without turning an
            # unverified room capability into a solver constraint.
            findings.append(_finding(
                "UNSUPPORTED_ROOM_CAPABILITY", "review", [source],
                "Требование к программам или оборудованию аудитории пока не проверяется и не будет учтено при составлении."
            ))
            continue

        if NEGATED_AVOIDANCE_INTENT.search(text):
            findings.append(_finding("AMBIGUOUS_PREFERENCE", "review", [source],
                                     "Фраза содержит отрицание пожелания избегать указанное время; уточните, как его учитывать."))
            continue

        if DOUBLE_NEGATION.search(text):
            findings.append(_finding("AMBIGUOUS_PREFERENCE", "review", [source],
                                     "В пожелании есть двойное отрицание; уточните, хочет ли преподаватель избегать этого времени."))
            continue

        if "подряд" in text:
            unavailable, available = _intents(text)
            if unavailable and available:
                findings.append(_finding("AMBIGUOUS_PREFERENCE", "review", [source],
                                         "Пожелание одновременно просит ставить и избегать занятий подряд."))
                continue
            if unavailable or not available:
                message = ("Избегание занятий подряд пока требует решения администратора." if unavailable else
                           "Не удалось определить, хочет ли преподаватель ставить занятия подряд.")
                findings.append(_finding("UNSUPPORTED_PREFERENCE", "review", [source],
                                         message))
                continue
            if _unsupported_consecutive_count(text):
                findings.append(_finding("UNSUPPORTED_PREFERENCE", "review", [source],
                                         "Пока поддерживается только пожелание о двух занятиях подряд; более длинная цепочка требует ручной проверки."))
                continue
            if _weekday_mentioned(text) or _period_mentioned(text):
                findings.append(_finding("UNSUPPORTED_PREFERENCE", "review", [source],
                                         "Пожелание о занятиях подряд содержит день или пару, которые пока нельзя учесть."))
                continue

            matches = ([row for row in teacher_requirements if row["key"] == scoped_requirement]
                       if scoped_requirement else
                       [row for row in teacher_requirements if _subject_matches(row["subject"], text)])
            if scoped_requirement and matches and SUBJECT_REFERENCE.search(text) and not _subject_matches(matches[0]["subject"], text):
                findings.append(_finding("UNSUPPORTED_PREFERENCE", "review", [source],
                                         "Указанный предмет не совпадает с выбранной записью нагрузки; проверьте формулировку."))
                continue
            if len(matches) == 1:
                proposals.append({"schema_version": 1, "type": "consecutive_lessons",
                                  "scope": {"teacher": teacher, "requirement": matches[0]["key"]},
                                  "pair_count": 1, "strength": "soft", "status": "proposed", "source": source})
            elif len(matches) > 1:
                findings.append(_finding("AMBIGUOUS_PREFERENCE", "review", [source],
                                         "Уточните предмет: преподаватель ведёт несколько занятий, и неизвестно, какие ставить подряд."))
            elif not teacher_requirements:
                findings.append(_finding("UNSUPPORTED_PREFERENCE", "review", [source],
                                         "В выбранном семестре нет нагрузки преподавателя для этого пожелания."))
            elif SUBJECT_REFERENCE.search(text):
                findings.append(_finding("UNSUPPORTED_PREFERENCE", "review", [source],
                                         "Указанный предмет не найден среди занятий преподавателя; уточните формулировку."))
            else:
                proposals.append({"schema_version": 1, "type": "consecutive_lessons",
                                  "scope": ({"teacher": teacher, "requirement": scoped_requirement}
                                            if scoped_requirement else {"teacher": teacher}),
                                  "pair_count": 1, "strength": "soft", "status": "proposed", "source": source})
            continue

        days = _weekdays_mentioned(text)
        slot_numbers = _period_numbers(text)
        if _week_type(text) is None:
            findings.append(_finding("AMBIGUOUS_PREFERENCE", "review", [source],
                                     "Пожелание одновременно указывает чётные и нечётные недели."))
            continue
        week_type = _week_type(text)
        if days and slot_numbers:
            period_reference = rf"(?:\b[1-9]\b|\b{ORDINAL_PATTERN}\b)"
            alternative_period = (
                (_period_mentioned(text) or len(set(slot_numbers)) > 1)
                and re.search(
                    rf"{period_reference}[^\n.!?;]{{0,24}}(?:\bили\b|\bлибо\b|/)"
                    rf"[^\n.!?;]{{0,24}}{period_reference}", text
                )
            )
            alternative_day = len(days) > 1 and re.search(r"\b(?:или|либо|а)\b", text)
            if alternative_day or alternative_period:
                findings.append(_finding("AMBIGUOUS_PREFERENCE", "review", [source],
                                         "Уточните, какой из перечисленных дней или пар выбрать; сейчас их нельзя различить."))
                continue
            unavailable, available = _intents(text)
            if unavailable and available:
                findings.append(_finding("AMBIGUOUS_PREFERENCE", "review", [source],
                                         "Пожелание одновременно выражает предпочтение и запрет для указанного времени."))
                continue
            if not unavailable and not available:
                findings.append(_finding("UNSUPPORTED_PREFERENCE", "review", [source],
                                         "Не удалось определить, хочет ли преподаватель избегать этого времени или предпочитает его."))
                continue

            slots = [f"p{number}" for number in sorted(set(slot_numbers))]
            available_slots = _available_slots(data)
            if not available_slots or not set(slots).issubset(available_slots):
                findings.append(_finding("UNSUPPORTED_PREFERENCE", "review", [source],
                                         "В расписании нет указанной пары; уточните номер существующего занятия."))
                continue
            rule_type = "unavailable_slot" if unavailable else "available_slot"
            opposite = "available_slot" if unavailable else "unavailable_slot"
            # Only an explicit hard profile unavailability can withhold a text
            # wish. A teacher's text-based unavailability remains a soft wish
            # even when it opposes a soft profile availability preference.
            profile_conflict = rule_type == "available_slot" and any(
                row["teacher"] == teacher and row["type"] == "unavailable_slot"
                and row["weekday"] in days and set(row["slots"]) & set(slots)
                and _weeks_overlap(row.get("week_type", "both"), week_type) for row in profile
            )
            if profile_conflict:
                findings.append(_finding("PROFILE_TEXT_CONTRADICTION", "review", [source],
                                         "Текст преподавателя противоречит ограничению в профиле."))
                continue

            proposals.extend({"schema_version": 1, "type": rule_type,
                              "scope": ({"teacher": teacher, "requirement": scoped_requirement}
                                        if scoped_requirement else {"teacher": teacher}), "weekday": day,
                              "slots": slots, "week_type": week_type,
                              "strength": "soft", "status": "proposed", "source": source}
                             for day in WEEKDAY_ORDER if day in days)
            continue

        if days and not slot_numbers and _period_mentioned(text):
            findings.append(_finding("UNSUPPORTED_PREFERENCE", "review", [source],
                                     "Не удалось распознать номер пары в пожелании."))
            continue

        findings.append(_finding("UNSUPPORTED_PREFERENCE", "review", [source],
                                 "Пожелание требует разбора администратором."))
    return {"proposals": proposals, "findings": findings}


def _period_numbers(text):
    numbers = [int(number) for number in re.findall(rf"\b([1-9])\s*{PAIR_NOUN}", text)]
    numbers.extend(int(number) for number in re.findall(rf"{PAIR_NOUN}\s*:?[ \t]*([1-9])\b", text))
    for first, last in re.findall(rf"\b([1-9])\s*[-‑–—]\s*([1-9])\s*{PAIR_NOUN}", text):
        if int(first) <= int(last):
            numbers.extend(range(int(first), int(last) + 1))
    for first, last in re.findall(rf"{PAIR_NOUN}\s*:?[ \t]*([1-9])\s*[-‑–—]\s*([1-9])\b", text):
        if int(first) <= int(last):
            numbers.extend(range(int(first), int(last) + 1))
    for phrase in re.findall(rf"\b[1-9](?:\s*(?:и|,|/)\s*[1-9])+\s*{PAIR_NOUN}", text):
        numbers.extend(int(number) for number in re.findall(r"[1-9]", phrase))
    for phrase in re.findall(rf"{PAIR_NOUN}\s*:?[ \t]*[1-9](?:\s*(?:и|,|/)\s*[1-9])+\b", text):
        numbers.extend(int(number) for number in re.findall(r"[1-9]", phrase))
    numbers.extend(int(number) for number in re.findall(
        rf"\b([1-9])[-‑–]?(?:я|й|ю|ую|ой|ая|ый)\b(?=.{{0,24}}{PAIR_NOUN})", text
    ))
    numbers.extend(number for number, pattern in ORDINALS.items()
                    if re.search(rf"\b{pattern}\b.{{0,24}}{PAIR_NOUN}", text))
    return numbers


def _available_slots(data):
    slots = data.get("available_slots")
    if slots is None:
        slots = [row.get("key") for row in data.get("time_slots", []) if isinstance(row, dict)]
    if not isinstance(slots, (list, tuple)):
        return set()
    return {str(slot) for slot in slots if isinstance(slot, str)}


def _unsupported_consecutive_count(text):
    if re.search(
        rf"\b(?:{PAIR_COUNT_PREFIX})\s+{NON_LESSON_COUNT_UNIT}\b"
        rf"[^.!?;\n]{{0,64}}\bподряд\b", text
    ):
        return True

    if re.search(
        rf"\b(?:{PAIR_COUNT_WORD})\s+с\s+половин\w*\b[^.!?;\n]{{0,48}}\bподряд\b", text
    ):
        return True

    if re.search(
        rf"{CONSECUTIVE_COUNT_QUALIFIER.pattern}\b(?:{PAIR_COUNT_PREFIX})\s+"
        rf"{CONSECUTIVE_LESSON_NOUN}.{{0,32}}\bподряд\b", text
    ):
        return True

    pattern = rf"\b({PAIR_COUNT_PREFIX})\s+([^.!?;\n]{{0,48}}?)\bподряд\b"
    for value, after_count in re.findall(pattern, text):
        numeric_count = re.fullmatch(r"(\d+)(?:[-‑–—][а-яё]+)?", value)
        is_two = int(numeric_count.group(1)) == 2 if numeric_count else value in TWO_CONSECUTIVE_COUNTS
        has_lesson_noun = re.search(CONSECUTIVE_LESSON_NOUN, after_count)
        has_additional_count = re.search(rf"\b(?:{PAIR_COUNT_PREFIX})\b", after_count)
        if not is_two or not has_lesson_noun or has_additional_count:
            return True

    if re.search(rf"\b{ORDINAL_PATTERN}\b[^.!?;\n]{{0,48}}?\bподряд\b", text):
        return True
    return bool(re.search(
        r"\b(?:нескольк\w*|мног\w*)\b[^.!?;\n]{0,48}?\bподряд\b", text
    ))


def _weekdays_mentioned(text):
    return {value for token, value in WEEKDAYS.items() if re.search(rf"\b{re.escape(token)}\b", text)}


def _week_type(text):
    even = bool(re.search(r"\bч[её]тн\w*", text))
    odd = bool(re.search(r"\bнеч[её]тн\w*", text))
    if even and odd:
        return None
    return "even" if even else "odd" if odd else "both"


def _finding(code, severity, sources, message):
    return {"code": code, "severity": severity, "sources": sources, "message": message}


def _teachers(requirement):
    return requirement.get("teachers") or [requirement["teacher"]]


def _subject_matches(subject, text):
    subject_tokens = re.findall(r"[а-яё0-9]+", subject.casefold().replace("ё", "е"))
    text_tokens = re.findall(r"[а-яё0-9]+", text.casefold().replace("ё", "е"))
    stems = [_subject_stem(token) for token in subject_tokens
             if len(token) >= 3 and token not in SUBJECT_STOP_WORDS]
    return bool(stems) and all(any(word.startswith(stem) for word in text_tokens) for stem in stems)


def _subject_stem(token):
    if token.endswith(("ый", "ий")) and len(token) > 4:
        return token[:-2]
    if token.endswith(("а", "я", "е", "ё", "и", "ы", "у", "ю", "о")) and len(token) > 3:
        return token[:-1]
    return token


def _weeks_overlap(left, right):
    return left == right or "both" in (left, right)


def _intents(text):
    unavailable = bool(UNAVAILABLE_INTENT.search(text))
    positive_text = UNAVAILABLE_INTENT.sub(" ", NEGATED_WISH_INTENT.sub(" ", text))
    available = bool(AVAILABLE_INTENT.search(positive_text))
    return unavailable, available


def _weekday_mentioned(text):
    return any(re.search(rf"\b{re.escape(token)}\b", text) for token in WEEKDAYS)


def _period_mentioned(text):
    has_numbered_period = bool(re.search(
        rf"\b(?:на|к|после|до|с)\s+[1-9](?:\s*[-‑–—]\s*[1-9])?\s*{PAIR_NOUN}", text
    ))
    has_ordinal_period = any(re.search(rf"\b{pattern}\b.{{0,24}}{PAIR_NOUN}", text) for pattern in ORDINALS.values())
    has_reversed_number = bool(re.search(rf"{PAIR_NOUN}\s*:?[ \t]*[1-9]", text))
    has_clock = bool(re.search(r"\b\d{1,2}:\d{2}\b", text))
    has_week_scope = bool(re.search(r"\b(?:неч[её]тн\w*|ч[её]тн\w*)\b", text))
    return has_numbered_period or has_ordinal_period or has_reversed_number or has_clock or has_week_scope
