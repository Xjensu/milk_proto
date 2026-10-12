"""Static checks shared by the sample proposal and generated output."""

from collections import defaultdict

from preference_compiler import compile_wishes


def week_overlap(left, right):
    return left == right or "both" in (left, right)


def analyze(data, proposal):
    requirements = {row["key"]: row for row in data["requirements"]}
    groups = {row["key"]: row for row in data["groups"]}
    rooms = {row["key"]: row for row in data["rooms"]}
    staged_event_ids = {
        str(source_id)
        for row in proposal if row.get("action") in ("update_existing", "move_dated_occurrence")
        for source_id in (row.get("source_event_ids", []) if row.get("action") == "update_existing"
                          else row.get("source_lesson_keys", []))
    }
    lessons = [
        ("published", row) for row in data.get("published_lessons", [])
        if str(row.get("key")) not in staged_event_ids
    ]
    staged_dates = {
        (str(event_id), row.get("source_date"))
        for row in proposal if row.get("action") == "move_dated_occurrence"
        for event_id in row.get("source_schedule_event_ids", [])
    }
    lessons += [
        ("published_occurrence", row) for row in data.get("date_occurrences", [])
        if (str(row.get("schedule_event_id")), row.get("date")) not in staged_dates
    ]
    lessons += [("proposal", row) for row in proposal]
    expanded_dates = {row.get("date") for row in data.get("date_occurrences", [])}
    findings = []

    for origin, lesson in lessons:
        if origin == "proposal" or lesson.get("in_scope"):
            linked_requirements = _requirements_for(lesson, requirements)
            lesson_teachers = _teacher_ids(lesson, linked_requirements, origin)
            for rule in data.get("profile_constraints", []):
                if rule["type"] == "unavailable_slot" and rule["teacher"] in lesson_teachers \
                        and rule["weekday"] == lesson["weekday"] and lesson["slot"] in rule["slots"] \
                        and week_overlap(rule.get("week_type", "both"), lesson.get("week_type", "both")):
                    findings.append(_finding("TEACHER_UNAVAILABLE", [lesson["key"]], [rule["key"]],
                                             "Преподаватель недоступен в это время."))
            for room_key in _room_ids(lesson):
                for reservation in data.get("room_reservations", []):
                    exact_date_match = lesson.get("date") and reservation.get("date") and \
                        lesson.get("date") == reservation.get("date")
                    if reservation["room"] == room_key and reservation["weekday"] == lesson["weekday"] \
                            and reservation["slot"] == lesson["slot"] \
                            and (not lesson.get("date") or not reservation.get("date") or
                                 reservation.get("date") == lesson.get("date")) \
                            and (exact_date_match or week_overlap(
                                reservation.get("week_type", "both"), lesson.get("week_type", "both")
                            )):
                        findings.append(_finding("ROOM_RESERVED", [lesson["key"]], [reservation["key"]],
                                                 "Аудитория забронирована в это время."))
            room = rooms.get(lesson.get("room")) if lesson.get("room") is not None else None
            required_students = sum(
                groups[row["group"]]["students"]
                for row in linked_requirements
            )
            assigned_rooms = [rooms.get(room_key) for room_key in _room_ids(lesson)]
            if assigned_rooms and all(item and item.get("capacity") is not None for item in assigned_rooms) \
                    and sum(item["capacity"] for item in assigned_rooms) < required_students:
                sources = [row["group"] for row in linked_requirements] + [item["key"] for item in assigned_rooms]
                findings.append(_finding("ROOM_CAPACITY", [lesson["key"]], sources,
                                         "Вместимость аудитории меньше общей численности студентов."))
            for linked_requirement in linked_requirements:
                group = groups[linked_requirement["group"]]
                if room and lesson["room"] not in linked_requirement["eligible_rooms"]:
                    findings.append(_finding("ROOM_NOT_ASSIGNED", [lesson["key"]], [linked_requirement["key"]],
                                             "Эта аудитория не закреплена за данной нагрузкой."))

    indexed_lessons = []
    resource_index = defaultdict(list)
    for origin, lesson in lessons:
        linked_requirements = _requirements_for(lesson, requirements)
        indexed = {
            "origin": origin,
            "lesson": lesson,
            "requirements": linked_requirements,
            "teachers": _teacher_ids(lesson, linked_requirements, origin),
            "groups": _group_ids(lesson, linked_requirements),
            "rooms": _room_ids(lesson),
        }
        index = len(indexed_lessons)
        indexed_lessons.append(indexed)
        cell = (lesson["weekday"], lesson["slot"])
        for resource_type, identities in (("teacher", indexed["teachers"]),
                                          ("group", indexed["groups"]), ("room", indexed["rooms"])):
            for identity in identities:
                resource_index[(*cell, resource_type, identity)].append(index)

    for index, left in enumerate(indexed_lessons):
        left_lesson = left["lesson"]
        cell = (left_lesson["weekday"], left_lesson["slot"])
        candidates = set()
        for resource_type, identities in (("teacher", left["teachers"]),
                                          ("group", left["groups"]), ("room", left["rooms"])):
            for identity in identities:
                candidates.update(resource_index[(*cell, resource_type, identity)])
        for other_index in sorted(candidate for candidate in candidates if candidate > index):
            right = indexed_lessons[other_index]
            if _pair_is_excluded(left, right, expanded_dates):
                continue
            right_lesson = right["lesson"]
            keys = [left_lesson["key"], right_lesson["key"]]
            if left["teachers"] & right["teachers"]:
                findings.append(_finding("TEACHER_CLASH", keys, [], "Преподаватель назначен на два занятия одновременно."))
            left_subgroup = left_lesson.get("subgroup", 0)
            right_subgroup = right_lesson.get("subgroup", 0)
            if left["groups"] & right["groups"] and \
                    (left_subgroup == 0 or right_subgroup == 0 or left_subgroup == right_subgroup):
                findings.append(_finding("GROUP_CLASH", keys, [], "У группы два занятия одновременно."))
            if left["rooms"] & right["rooms"]:
                findings.append(_finding("ROOM_CLASH", keys, [], "Аудитория занята двумя занятиями одновременно."))

    findings.extend(_subgroup_workload_findings(data, proposal))
    findings.extend(_approved_preference_findings(data, lessons, requirements))
    findings.extend(compile_wishes(data)["findings"])
    return findings


def _pair_is_excluded(left, right, expanded_dates):
    origin, lesson = left["origin"], left["lesson"]
    other_origin, other = right["origin"], right["lesson"]
    # An expanded calendar occurrence represents one exact date. Its recurring
    # source template is retained for weekly analysis, so skip that duplicate view.
    if {origin, other_origin} == {"published", "published_occurrence"}:
        return True
    if origin == "published" and other_origin == "proposal" and other.get("date") in expanded_dates:
        return True
    if other_origin == "published" and origin == "proposal" and lesson.get("date") in expanded_dates:
        return True
    if origin == "published_occurrence" and other_origin == "proposal" and not other.get("date"):
        return True
    if other_origin == "published_occurrence" and origin == "proposal" and not lesson.get("date"):
        return True
    if lesson.get("date") and other.get("date") and lesson["date"] != other["date"]:
        return True
    exact_date_match = lesson.get("date") and other.get("date") and lesson["date"] == other["date"]
    if not exact_date_match and not week_overlap(
        lesson.get("week_type", "both"), other.get("week_type", "both")
    ):
        return True
    if origin == other_origin == "published":
        same_batch = lesson.get("event_batch_id") and lesson.get("event_batch_id") == other.get("event_batch_id")
        if same_batch or (not lesson.get("in_scope") and not other.get("in_scope")):
            return True
    if origin == "proposal" and lesson.get("action") == "move_dated_occurrence" and \
            str(other.get("key")) in set(lesson.get("source_lesson_keys", [])):
        return True
    if other_origin == "proposal" and other.get("action") == "move_dated_occurrence" and \
            str(lesson.get("key")) in set(other.get("source_lesson_keys", [])):
        return True
    if origin == "proposal" and lesson.get("action") == "move_dated_occurrence" and \
            other_origin == "published_occurrence" and \
            str(other.get("schedule_event_id")) in set(map(str, lesson.get("source_schedule_event_ids", []))) and \
            other.get("date") == lesson.get("source_date"):
        return True
    if other_origin == "proposal" and other.get("action") == "move_dated_occurrence" and \
            origin == "published_occurrence" and \
            str(lesson.get("schedule_event_id")) in set(map(str, other.get("source_schedule_event_ids", []))) and \
            lesson.get("date") == other.get("source_date"):
        return True
    return False


def _approved_preference_findings(data, lessons, requirements):
    preferences = [row for row in data.get("approved_preferences", [])
                   if row.get("status") == "approved"]
    if not preferences:
        return []

    # A recurring template and its date-expanded occurrence are two views of the
    # same event. Keep one copy when checking a weekly wish.
    scheduled = []
    seen_events = set()
    for origin, lesson in lessons:
        if origin not in ("published", "published_occurrence", "proposal"):
            continue
        event_id = lesson.get("schedule_event_id")
        identity = (str(event_id), str(lesson.get("date"))) if event_id and lesson.get("date") else None
        if identity and identity in seen_events:
            continue
        if identity:
            seen_events.add(identity)
        linked = _requirements_for(lesson, requirements)
        scheduled.append((origin, lesson, linked, _teacher_ids(lesson, linked, origin)))

    slots = sorted(data.get("time_slots", []), key=lambda row: (row.get("number", 0), str(row.get("key"))))
    slot_positions = {str(row["key"]): index for index, row in enumerate(slots)}
    findings = []

    for rule in preferences:
        scope = rule.get("scope") or {}
        teacher = str(rule.get("teacher") or scope.get("teacher") or "")
        source = str(rule.get("source") or "")
        rule_type = rule.get("type")
        requirement_key = scope.get("requirement")
        matching = [row for row in scheduled
                    if teacher and teacher in row[3]
                    and (requirement_key is None or any(
                        str(requirement.get("key")) == str(requirement_key) for requirement in row[2]
                    ))]

        if rule_type in ("available_slot", "unavailable_slot"):
            slots_wanted = set(map(str, rule.get("slots", [])))
            week_type = rule.get("week_type", "both")

            def matches_slot(row):
                lesson = row[1]
                return (lesson.get("weekday") == rule.get("weekday")
                        and str(lesson.get("slot")) in slots_wanted
                        and week_overlap(week_type, lesson.get("week_type", "both")))

            matched = [row for row in matching if matches_slot(row)]
            violated = bool(matched) if rule_type == "unavailable_slot" else not matched
            if violated:
                affected = [str(row[1].get("key")) for row in (matched if matched else matching)]
                findings.append(_finding(
                    "TEACHER_SOFT_PREFERENCE_NOT_MET", affected, [source],
                    "В расписании не удалось учесть утверждённое пожелание преподавателя. Это мягкое пожелание; оно не блокирует публикацию.",
                    severity="review"
                ))
            continue

        if rule_type != "consecutive_lessons":
            continue
        pair_count = max(1, int(rule.get("pair_count", 1)))
        pairs_by_week = {"even": set(), "odd": set()}
        for index, left in enumerate(matching):
            for right in matching[index + 1:]:
                left_lesson, right_lesson = left[1], right[1]
                left_position = slot_positions.get(str(left_lesson.get("slot")))
                right_position = slot_positions.get(str(right_lesson.get("slot")))
                if left_position is None or right_position is None \
                        or abs(left_position - right_position) != 1 \
                        or left_lesson.get("weekday") != right_lesson.get("weekday") \
                        or str(left_lesson.get("key")) == str(right_lesson.get("key")):
                    continue
                if left_lesson.get("date") and right_lesson.get("date") \
                        and left_lesson.get("date") != right_lesson.get("date"):
                    continue
                for week in ("even", "odd"):
                    if week_overlap(week, left_lesson.get("week_type", "both")) \
                            and week_overlap(week, right_lesson.get("week_type", "both")):
                        pair_key = tuple(sorted((str(left_lesson.get("key")), str(right_lesson.get("key")))))
                        pairs_by_week[week].add(pair_key)
        if not any(len(pairs) >= pair_count for pairs in pairs_by_week.values()):
            affected = sorted({str(row[1].get("key")) for row in matching})
            findings.append(_finding(
                "TEACHER_SOFT_PREFERENCE_NOT_MET", affected, [source],
                "В расписании нет требуемого количества занятий подряд из утверждённого пожелания. Это мягкое пожелание; оно не блокирует публикацию.",
                severity="review"
            ))
    return findings


def _subgroup_workload_findings(data, proposal):
    events = {str(row.get("key")): row for row in data.get("published_lessons", [])}
    requirements = {str(row["key"]): row for row in data.get("requirements", [])}
    deltas = {}
    affected = {}

    for placement in proposal:
        if placement.get("action") != "update_existing":
            continue
        placement_key = str(placement.get("key"))
        target_subgroup = int(placement.get("subgroup", 0))
        for event_id in map(str, placement.get("source_event_ids", [])):
            source = events.get(event_id)
            if not source or source.get("dated"):
                continue
            source_subgroup = int(source.get("subgroup", 0))
            if source_subgroup == target_subgroup:
                continue
            requirement_keys = list(map(str, source.get("requirements") or [source["requirement"]]))
            for requirement_key in requirement_keys:
                requirement = requirements.get(requirement_key)
                if not requirement or set(map(int, requirement.get("subgroups", []))) != {1, 2}:
                    continue
                hours = int(requirement.get("hours_per_event", 0))
                if hours <= 0:
                    continue
                half_week_credit = hours if source.get("week_type", "both") in ("even", "odd") else hours * 2
                deltas[(requirement_key, source_subgroup)] = deltas.get((requirement_key, source_subgroup), 0) - half_week_credit
                deltas[(requirement_key, target_subgroup)] = deltas.get((requirement_key, target_subgroup), 0) + half_week_credit
                affected.setdefault(requirement_key, set()).add(placement_key)

    unbalanced = {key for key, delta in deltas.items() if delta != 0}
    findings = []
    for requirement_key in sorted({key[0] for key in unbalanced}):
        findings.append(_finding(
            "SUBGROUP_WORKLOAD_IMBALANCE",
            sorted(affected.get(requirement_key, set())),
            [requirement_key],
            "Изменения подгрупп должны сохранять часы обеих частей; перенесите компенсирующие занятия вместе."
        ))
    return findings


def _finding(code, lessons, sources, message, severity="error"):
    return {"code": code, "severity": severity, "lessons": lessons, "sources": sources, "message": message}


def _teachers(requirement):
    return requirement.get("teachers") or [requirement["teacher"]]


def _requirements_for(lesson, requirements):
    keys = lesson.get("requirements") or [lesson["requirement"]]
    return [requirements[key] for key in keys if key in requirements]


def _group_ids(lesson, requirements):
    return {str(key) for key in lesson.get("group_keys", [])} or {
        str(row["group"]) for row in requirements
    }


def _room_ids(lesson):
    return {str(key) for key in lesson.get("rooms", [lesson.get("room")]) if key is not None}


def _teacher_ids(lesson, requirements, origin):
    if lesson.get("teacher_ids"):
        return {str(teacher) for teacher in lesson["teacher_ids"]}
    if origin == "proposal":
        # Proposal `teachers` values are presentation labels. Resolve identities
        # from the linked workload requirements when checking keyed rules.
        return {str(teacher) for row in requirements for teacher in _teachers(row)}
    if lesson.get("teachers"):
        return {str(teacher) for teacher in lesson["teachers"]}
    return {str(teacher) for row in requirements for teacher in _teachers(row)}
