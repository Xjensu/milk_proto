"""Deterministic local CP-SAT scheduler for a faculty scheduling snapshot."""

from collections import defaultdict
import math
import random
import threading
import time

from ortools.sat.python import cp_model

from analyzer import analyze


DAYS = ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday"]


def generate(data, *, seconds=None, on_progress=None, cancel_event=None):
    if seconds is not None and int(seconds) < 1:
        raise ValueError("solve time limit must be positive")
    if _lease_lost(cancel_event):
        return _lease_lost_result()
    rooms = {row["key"]: row for row in data["rooms"]}
    groups = {row["key"]: row for row in data["groups"]}
    slot_labels = {row["key"]: f"Пара {row['number']} · {row['start']}–{row['end']}" for row in data["time_slots"]}
    teacher_labels = {row["key"]: row["label"] for row in data["teachers"]}
    slots = sorted(data["time_slots"], key=lambda slot: slot["number"])
    existing = data.get("published_lessons", [])
    requirements = {row["key"]: row for row in data["requirements"]}
    preferences = data.get("approved_preferences", [])
    existing_count = defaultdict(int)
    for index, lesson in enumerate(existing):
        if index % 256 == 0 and _lease_lost(cancel_event):
            return _lease_lost_result()
        existing_count[lesson["requirement"]] += 1
    blockers = _PlacementBlockers(data, existing, requirements, cancel_event=cancel_event)
    if blockers.cancelled or _lease_lost(cancel_event):
        return _lease_lost_result()
    placement_costs = _PlacementCosts(data, preferences)
    if _lease_lost(cancel_event):
        return _lease_lost_result()

    units = []
    for requirement in data.get("solver_requirements", data["requirements"]):
        if _lease_lost(cancel_event):
            return _lease_lost_result()
        if requirement.get("lesson_units") is None and requirement.get("week_type") not in ("both", "even", "odd"):
            return {"status": "unsupported", "reason": "UNSUPPORTED_WEEK_TYPE", "requirement": requirement["key"],
                    "placements": [], "findings": []}
        count = max(0, requirement["weekly_meetings"] - existing_count[requirement["key"]])
        if requirement.get("lesson_units") is not None:
            units.extend((requirement, lesson_unit) for lesson_unit in requirement["lesson_units"])
        else:
            units.extend((requirement, {"ordinal": index + 1, "week_types": [requirement["week_type"]]})
                         for index in range(count))

    group_unit_counts = defaultdict(int)
    flexible_week_groups = set()
    for requirement, lesson_unit in units:
        week_types = lesson_unit.get("week_types", [requirement.get("week_type", "both")])
        for group_key in requirement.get("group_keys", [requirement["group"]]):
            group_unit_counts[group_key] += 1
            if len(week_types) > 1:
                flexible_week_groups.add(group_key)
    compact_groups = {group for group, count in group_unit_counts.items()
                      if count > 1 or group in flexible_week_groups}

    model = cp_model.CpModel()
    choices = {}
    indexes = defaultdict(list)
    occupancies = defaultdict(list)
    preference_costs = []
    period_costs = []
    compact_cells = defaultdict(list)
    period_cost_bound = 0
    preference_cost_bound = 0
    assigned_units = []
    candidate_rows_by_unit = []
    for unit_index, (requirement, lesson_unit) in enumerate(units):
        if _lease_lost(cancel_event):
            return _lease_lost_result()
        candidates = []
        candidate_rows = []
        unit_period_cost_bound = 0
        unit_preference_cost_bound = 0
        eligible_rooms = requirement["eligible_rooms"] or [None]
        week_types = lesson_unit.get("week_types", [requirement.get("week_type", "both")])
        for day in DAYS:
            if _lease_lost(cancel_event):
                return _lease_lost_result()
            for slot in slots:
                if _lease_lost(cancel_event):
                    return _lease_lost_result()
                for room_key in eligible_rooms:
                    room = rooms.get(room_key) if room_key is not None else None
                    group_sizes = requirement.get("group_sizes", [groups[requirement["group"]]["students"]])
                    if room and room.get("capacity") is not None and room["capacity"] < sum(group_sizes):
                        continue
                    for week_type in week_types:
                        if week_type not in ("both", "even", "odd"):
                            continue
                        subgroup = lesson_unit.get("subgroup", 0)
                        if blockers.blocks(requirement, day, slot["key"], room_key, week_type, subgroup):
                            continue
                        key = (unit_index, day, slot["key"], room_key, week_type)
                        variable = model.NewBoolVar(f"u{unit_index}_{day}_{slot['key']}_{room_key}_{week_type}")
                        choices[key] = variable
                        candidates.append(variable)
                        for week in ("even", "odd") if week_type == "both" else (week_type,):
                            occupancies[(unit_index, day, slot["key"], week)].append(variable)
                        resources = [("teacher", teacher) for teacher in _teachers(requirement)]
                        for group_key in requirement.get("group_keys", [requirement["group"]]):
                            resources.extend(("group", f"{group_key}:{overlap_group}")
                                             for overlap_group in _overlap_group_subgroups(subgroup))
                        if room_key is not None:
                            resources.append(("room", room_key))
                        resource_tokens = []
                        for resource_type, resource_key in resources:
                            weeks = ("even", "odd") if week_type == "both" else (week_type,)
                            for week in weeks:
                                token = (resource_type, resource_key, day, slot["key"], week)
                                indexes[token].append(variable)
                                resource_tokens.append(token)
                        candidate_rows.append((key, variable, tuple(resource_tokens)))
                        slot_cost = slot["number"] - 1
                        period_costs.append(slot_cost * variable)
                        unit_period_cost_bound = max(unit_period_cost_bound, abs(slot_cost))
                        preference_cost = placement_costs.cost(requirement, day, slot["key"], week_type)
                        if preference_cost:
                            preference_costs.append(preference_cost * variable)
                            unit_preference_cost_bound = max(unit_preference_cost_bound, abs(preference_cost))
                        for group_key in requirement.get("group_keys", [requirement["group"]]):
                            if group_key not in compact_groups:
                                continue
                            for week in ("even", "odd") if week_type == "both" else (week_type,):
                                compact_cells[(group_key, day, week, slot["key"])].append(variable)
        assigned = model.NewBoolVar(f"assigned_unit_{unit_index}")
        model.Add(sum(candidates) == assigned)
        assigned_units.append(assigned)
        candidate_rows_by_unit.append(candidate_rows)
        period_cost_bound += unit_period_cost_bound
        preference_cost_bound += unit_preference_cost_bound

    for variables in indexes.values():
        if _lease_lost(cancel_event):
            return _lease_lost_result()
        model.AddAtMostOne(variables)

    occupied = {}
    for key, variables in occupancies.items():
        if _lease_lost(cancel_event):
            return _lease_lost_result()
        presence = model.NewBoolVar(f"occupied_{key[0]}_{key[1]}_{key[2]}_{key[3]}")
        model.Add(sum(variables) == presence)
        occupied[key] = presence

    # Proposed parser output cannot influence generation until an administrator approves it.
    for preference_index, rule in enumerate(preferences):
        if _lease_lost(cancel_event):
            return _lease_lost_result()
        if rule["type"] != "consecutive_lessons":
            continue
        scope = rule.get("scope", {})
        teacher = scope.get("teacher")
        requirement_key = scope.get("requirement")
        matching_units = [index for index, (requirement, _) in enumerate(units)
                          if (requirement_key is None or requirement_key in requirement.get(
                              "requirement_keys", [requirement["key"]]))
                          and (teacher is None or teacher in _teachers(requirement))]
        if len(matching_units) < 2:
            continue
        rule_pairs_by_week = defaultdict(list)
        if teacher is not None:
            # Teacher resource constraints guarantee at most one matching unit per cell.
            teacher_occupancies = defaultdict(list)
            for unit_index in matching_units:
                if _lease_lost(cancel_event):
                    return _lease_lost_result()
                for day in DAYS:
                    for slot in slots:
                        for week in ("even", "odd"):
                            presence = occupied.get((unit_index, day, slot["key"], week))
                            if presence is not None:
                                teacher_occupancies[(day, slot["key"], week)].append(presence)
            adjacent_pairs = {}
            for (day, slot_key, week), presences in teacher_occupancies.items():
                if _lease_lost(cancel_event):
                    return _lease_lost_result()
                if len(presences) == 1:
                    adjacent_pairs[(day, slot_key, week)] = presences[0]
                else:
                    presence = model.NewBoolVar(f"teacher_occupied_{teacher}_{day}_{slot_key}_{week}")
                    model.Add(sum(presences) == presence)
                    adjacent_pairs[(day, slot_key, week)] = presence
            for day in DAYS:
                if _lease_lost(cancel_event):
                    return _lease_lost_result()
                for slot_index, slot in enumerate(slots[:-1]):
                    next_slot = slots[slot_index + 1]
                    for week in ("even", "odd"):
                        a = adjacent_pairs.get((day, slot["key"], week))
                        b = adjacent_pairs.get((day, next_slot["key"], week))
                        if a is None or b is None:
                            continue
                        pair = model.NewBoolVar(f"consecutive_teacher_{teacher}_{day}_{slot['key']}_{week}")
                        model.Add(pair <= a)
                        model.Add(pair <= b)
                        model.Add(pair >= a + b - 1)
                        rule_pairs_by_week[week].append(pair)
        else:
            for day in DAYS:
                if _lease_lost(cancel_event):
                    return _lease_lost_result()
                for slot_index, slot in enumerate(slots[:-1]):
                    next_slot = slots[slot_index + 1]
                    for week in ("even", "odd"):
                        for offset, left in enumerate(matching_units):
                            for right in matching_units[offset + 1:]:
                                for first, second in ((left, right), (right, left)):
                                    a = occupied.get((first, day, slot["key"], week))
                                    b = occupied.get((second, day, next_slot["key"], week))
                                    if a is None or b is None:
                                        continue
                                    pair = model.NewBoolVar(
                                        f"consecutive_{first}_{second}_{day}_{slot['key']}_{week}"
                                    )
                                    model.Add(pair <= a)
                                    model.Add(pair <= b)
                                    model.Add(pair >= a + b - 1)
                                    rule_pairs_by_week[week].append(pair)
        for rule_pairs in rule_pairs_by_week.values():
            if rule_pairs:
                reward_limit = max(1, int(rule.get("pair_count", 1)))
                reward_bound = min(reward_limit, len(rule_pairs))
                rewarded_pairs = model.NewIntVar(
                    0, reward_bound, f"consecutive_reward_{preference_index}_{len(preference_costs)}"
                )
                model.Add(rewarded_pairs <= sum(rule_pairs))
                preference_costs.append(-20 * rewarded_pairs)
                preference_cost_bound += 20 * reward_bound

    compact_costs = []
    compact_cost_bound = 0
    cells_by_cohort = defaultdict(dict)
    for (group_key, day, week, slot_key), variables in compact_cells.items():
        cells_by_cohort[(group_key, day, week)][slot_key] = variables
    slot_keys = [slot["key"] for slot in slots]
    for cohort, cells in cells_by_cohort.items():
        presences = []
        for slot_key in slot_keys:
            variables = cells.get(slot_key, [])
            if variables:
                presence = model.NewBoolVar(f"compact_cell_{len(presences)}_{cohort}")
                model.AddMaxEquality(presence, variables)
            else:
                presence = model.NewConstant(0)
            presences.append(presence)

        active_day = model.NewBoolVar(f"compact_day_{cohort}")
        model.AddMaxEquality(active_day, presences)
        compact_costs.append(active_day)
        compact_cost_bound += 1

        before = []
        for index, presence in enumerate(presences):
            prefix = model.NewBoolVar(f"occupied_before_{index + 1}_{cohort}")
            if index == 0:
                model.Add(prefix == presence)
            else:
                model.AddMaxEquality(prefix, [before[-1], presence])
            before.append(prefix)

        after = [None] * len(presences)
        for index in reversed(range(len(presences))):
            suffix = model.NewBoolVar(f"occupied_after_{index}_{cohort}")
            if index == len(presences) - 1:
                model.Add(suffix == presences[index])
            else:
                model.AddMaxEquality(suffix, [after[index + 1], presences[index]])
            after[index] = suffix

        for index in range(1, len(presences) - 1):
            gap = model.NewBoolVar(f"gap_{index}_{cohort}")
            model.Add(gap >= before[index - 1] + after[index + 1] - presences[index] - 1)
            compact_costs.append(gap)
            compact_cost_bound += 1

    # Keep the objective lexicographic: required placements, approved teacher
    # wishes, compact days/gaps, then earlier periods as a final tie-breaker.
    period_priority = 1
    compact_priority = period_cost_bound + 1
    preference_priority = compact_cost_bound * compact_priority + period_cost_bound + 1
    placement_priority = (2 * preference_cost_bound * preference_priority
                          + compact_cost_bound * compact_priority + period_cost_bound + 1)
    model.Minimize(
        sum((1 - assigned) * placement_priority for assigned in assigned_units)
        + sum(cost * preference_priority for cost in preference_costs)
        + sum(cost * compact_priority for cost in compact_costs)
        + sum(cost * period_priority for cost in period_costs)
    )
    if _lease_lost(cancel_event):
        return _lease_lost_result()
    _add_greedy_hints(model, assigned_units, units, candidate_rows_by_unit, slots,
                      placement_costs, cancel_event)
    if _lease_lost(cancel_event):
        return _lease_lost_result()
    solver = cp_model.CpSolver()
    if seconds is not None:
        solver.parameters.max_time_in_seconds = float(seconds)
    solver.parameters.num_search_workers = 1
    solver.parameters.random_seed = 7
    solver.parameters.repair_hint = True
    progress = _SearchProgress(list(choices.items()), len(units), on_progress) if on_progress else None
    if on_progress:
        bound_updates = 0
        search_started_at = time.monotonic()
        progress_phase = "preparing"

        def report_solver_log(message):
            nonlocal progress_phase
            line = message.strip()
            next_phase = None
            if line.startswith("Starting presolve at"):
                next_phase = "presolving"
            elif line.startswith("Starting search at"):
                next_phase = "searching"
            if next_phase is None or next_phase == progress_phase:
                return

            progress_phase = next_phase
            on_progress({"phase": next_phase, "solutions": 0, "placed": 0, "total": len(units),
                         "branches": 0, "conflicts": 0,
                         "elapsed_seconds": round(time.monotonic() - search_started_at, 2)})

        # CP-SAT's structured callbacks begin after presolve. Its log callback
        # supplies only lifecycle markers here; the model and solver log remain private.
        solver.parameters.log_search_progress = True
        solver.parameters.log_to_stdout = False
        solver.log_callback = report_solver_log

        def report_best_bound(bound):
            nonlocal bound_updates
            if not math.isfinite(bound):
                return
            bound_updates += 1
            on_progress({"phase": "searching", "bound_updates": bound_updates,
                         "objective_bound": round(bound, 3), "total": len(units),
                         "elapsed_seconds": round(time.monotonic() - search_started_at, 2)})

        solver.best_bound_callback = report_best_bound
    if on_progress:
        on_progress({"phase": "preparing", "solutions": 0, "placed": 0, "total": len(units), "branches": 0,
                     "conflicts": 0, "elapsed_seconds": 0.0})
    monitor_stop = threading.Event()
    cancel_monitor = None
    if cancel_event is not None:
        if _lease_lost(cancel_event):
            return _lease_lost_result()

        def stop_if_lease_lost():
            while not monitor_stop.wait(0.1):
                if cancel_event.is_set():
                    solver.StopSearch()
                    return

        cancel_monitor = threading.Thread(target=stop_if_lease_lost, daemon=True)
        cancel_monitor.start()
    try:
        status = solver.Solve(model, progress) if progress else solver.Solve(model)
    finally:
        monitor_stop.set()
        if cancel_monitor is not None:
            cancel_monitor.join(timeout=1.0)
    if _lease_lost(cancel_event):
        return _lease_lost_result()
    placed_count = sum(solver.Value(assigned) for assigned in assigned_units) \
        if status in (cp_model.OPTIMAL, cp_model.FEASIBLE) else 0
    solver_progress = {
        "solutions": progress.solutions if progress else 0,
        "placed": placed_count,
        "total": len(units), "branches": solver.NumBranches(), "conflicts": solver.NumConflicts(),
        "elapsed_seconds": round(solver.WallTime(), 2),
    }
    if status in (cp_model.OPTIMAL, cp_model.FEASIBLE):
        objective = solver.ObjectiveValue()
        bound = solver.BestObjectiveBound()
        if math.isfinite(objective):
            solver_progress["best_objective"] = round(objective, 3)
        if math.isfinite(bound):
            solver_progress["objective_bound"] = round(bound, 3)
    if on_progress:
        on_progress({**solver_progress, "phase": "validating"})
    if status not in (cp_model.OPTIMAL, cp_model.FEASIBLE):
        result = {"status": "infeasible" if status == cp_model.INFEASIBLE else "unknown",
                  "placements": [], "findings": [], "solver_progress": solver_progress}
        if status == cp_model.INFEASIBLE:
            result["reason"] = "MODEL_INFEASIBLE"
            result["findings"] = [{
                "code": "MODEL_INFEASIBLE",
                "severity": "error",
                "lesson_units": [],
                "requirements": [],
                "message": "Алгоритм не нашёл решение даже при пропуске отдельных занятий. Проверьте исходную нагрузку и создайте черновик повторно.",
            }]
        return result

    placements = []
    for (unit_index, day, slot, room, week_type), variable in choices.items():
        if solver.Value(variable):
            requirement, lesson_unit = units[unit_index]
            number = lesson_unit.get("ordinal", unit_index + 1)
            group = groups[requirement["group"]]
            room_label = rooms.get(room, {}).get("label") if room else None
            placements.append({"key": f"generated-{requirement['key']}-{number}",
                               "requirement": requirement["key"],
                               "requirements": requirement.get("requirement_keys", [requirement["key"]]),
                               "weekday": day, "slot": slot,
                               "room": room, "week_type": week_type, "lesson_unit_ordinal": number,
                               "subgroup": lesson_unit.get("subgroup", 0),
                               "group_label": requirement.get("group_label", group["label"]), "subject": requirement["subject"],
                               "activity": requirement.get("activity", ""),
                               "teachers": [teacher_labels.get(key, "Имя не указано")
                                            for key in _teachers(requirement)],
                               "slot_label": slot_labels[slot], "room_label": room_label})
    placements.sort(key=lambda row: (DAYS.index(row["weekday"]), row["slot"], row["key"]))
    findings = analyze(data, placements)
    unplaced = [index for index, assigned in enumerate(assigned_units) if not solver.Value(assigned)]
    for unit_index in unplaced:
        requirement, lesson_unit = units[unit_index]
        group = groups.get(requirement["group"], {})
        unit = {
            "requirement": requirement["key"],
            "requirements": requirement.get("requirement_keys", [requirement["key"]]),
            "group": requirement.get("group_label", group.get("label", "Группа")),
            "subject": requirement["subject"],
            "ordinal": lesson_unit.get("ordinal", unit_index + 1),
            "subgroup": lesson_unit.get("subgroup", 0),
        }
        label = f"{unit['group']} · {unit['subject']} · занятие {unit['ordinal']}"
        findings.append({
            "code": "UNPLACED_REQUIRED_LESSON",
            "severity": "error",
            "lessons": [],
            "lesson_units": [unit],
            "requirements": unit["requirements"],
            "message": (f"Не удалось разместить обязательное занятие: {label}. Оно отсутствует в черновике. "
                        "Проверьте исходные ограничения и создайте новый черновик; публикация заблокирована."),
        })
    if _lease_lost(cancel_event):
        return _lease_lost_result()
    if on_progress:
        on_progress({**solver_progress, "phase": "complete"})
    return {"status": "optimal" if status == cp_model.OPTIMAL else "feasible", "placements": placements,
            "objective": solver.ObjectiveValue(), "findings": findings, "solver_progress": solver_progress}


class _SearchProgress(cp_model.CpSolverSolutionCallback):
    def __init__(self, choices, total, reporter):
        super().__init__()
        self.choices = choices
        self.total = total
        self.reporter = reporter
        self.solutions = 0
        self.last_report = -2.0

    def on_solution_callback(self):
        self.solutions += 1
        elapsed = self.WallTime()
        if self.solutions > 1 and elapsed - self.last_report < 1.5:
            return
        self.last_report = elapsed
        placed = len({unit for (unit, *_), variable in self.choices if self.Value(variable)})
        objective = self.ObjectiveValue()
        bound = self.BestObjectiveBound()
        progress = {
            "phase": "searching",
            "solutions": self.solutions, "placed": placed, "total": self.total,
            "branches": self.NumBranches(), "conflicts": self.NumConflicts(),
            "elapsed_seconds": round(elapsed, 2),
        }
        if math.isfinite(objective):
            progress["best_objective"] = round(objective, 3)
        if math.isfinite(bound):
            progress["objective_bound"] = round(bound, 3)
        self.reporter(progress)


class _PlacementBlockers:
    """Index fixed occupancy once instead of rescanning the snapshot per candidate."""

    def __init__(self, data, existing, requirements, *, cancel_event=None):
        self.cancelled = False
        self.any_week = set()
        self.by_week = set()
        self.both_week = set()

        for rule in data.get("profile_constraints", []):
            if _lease_lost(cancel_event):
                self.cancelled = True
                return
            if rule["type"] != "unavailable_slot":
                continue
            for slot in rule["slots"]:
                self._add("teacher", rule["teacher"], rule["weekday"], slot, rule.get("week_type", "both"))

        for reservation in data.get("room_reservations", []):
            if _lease_lost(cancel_event):
                self.cancelled = True
                return
            self._add("room", reservation["room"], reservation["weekday"], reservation["slot"],
                      reservation.get("week_type", "both"))

        for index, lesson in enumerate(existing):
            if index % 256 == 0 and _lease_lost(cancel_event):
                self.cancelled = True
                return
            requirement = requirements.get(lesson["requirement"])
            teachers = lesson.get("teachers") or (_teachers(requirement) if requirement else [])
            groups = {str(key) for key in lesson.get("group_keys", [])}
            if not groups and requirement:
                groups = {str(key) for key in requirement.get("group_keys", [requirement["group"]])}
            subgroup = lesson.get("subgroup", 0)
            week_type = lesson.get("week_type", "both")
            day, slot = lesson["weekday"], lesson["slot"]
            for teacher in teachers:
                self._add("teacher", teacher, day, slot, week_type)
            for group in groups:
                for overlap_group in _overlap_group_subgroups(subgroup):
                    self._add("group", f"{group}:{overlap_group}", day, slot, week_type)
            for room in lesson.get("rooms") or [lesson.get("room")]:
                self._add("room", room, day, slot, week_type)

    def _add(self, resource_type, identity, day, slot, week_type):
        key = (resource_type, identity, day, slot)
        self.any_week.add(key)
        if week_type == "both":
            self.both_week.add(key)
            for week in ("even", "odd"):
                self.by_week.add((*key, week))
        else:
            self.by_week.add((*key, week_type))

    def blocks(self, requirement, day, slot, room, week_type, subgroup):
        if week_type == "both":
            blocked = lambda kind, identity: (kind, identity, day, slot) in self.any_week
        else:
            blocked = lambda kind, identity: (
                (kind, identity, day, slot) in self.both_week or
                (kind, identity, day, slot, week_type) in self.by_week
            )

        if any(blocked("teacher", teacher) for teacher in _teachers(requirement)):
            return True
        if any(blocked("group", f"{group}:{overlap_group}")
               for group in requirement.get("group_keys", [requirement["group"]])
               for overlap_group in _overlap_group_subgroups(subgroup)):
            return True
        return room is not None and blocked("room", room)


class _PlacementCosts:
    """Index the existing soft rule weights by affected placement cell."""

    def __init__(self, data, preferences):
        self.by_cell = defaultdict(int)
        rules = [
            (rule, -12) for rule in data.get("profile_constraints", [])
            if rule.get("type") == "available_slot"
        ]
        rules.extend(
            (rule, -12 if rule.get("type") == "available_slot" else 120)
            for rule in preferences
            if rule.get("type") in ("available_slot", "unavailable_slot")
        )
        for rule, weight in rules:
            teacher = rule.get("teacher") or rule.get("scope", {}).get("teacher")
            if teacher is None:
                continue
            day = rule.get("weekday")
            rule_week = rule.get("week_type", "both")
            for slot in rule.get("slots", []):
                for candidate_week in ("both", "even", "odd"):
                    if _weeks_overlap(rule_week, candidate_week):
                        self.by_cell[(teacher, day, slot, candidate_week, rule.get("scope", {}).get("requirement"))] += weight

    def cost(self, requirement, day, slot, week_type):
        requirement_keys = {str(key) for key in requirement.get("requirement_keys", [requirement["key"]])}
        return sum(
            self.by_cell.get((teacher, day, slot, week_type, None), 0) +
            sum(self.by_cell.get((teacher, day, slot, week_type, key), 0) for key in requirement_keys)
            for teacher in set(_teachers(requirement))
        )


def _weeks_overlap(left, right):
    return left == right or "both" in (left, right)


def _add_greedy_hints(model, assigned_units, units, candidate_rows_by_unit, slots,
                      placement_costs, cancel_event):
    """Seed CP-SAT with deterministic, conflict-free partial schedules."""
    if not units:
        return

    slot_order = {slot["key"]: index for index, slot in enumerate(slots)}
    best_choices = {}
    best_score = float("inf")
    best_count = -1

    for attempt in range(5):
        if _lease_lost(cancel_event):
            return
        rng = random.Random(7 + attempt)
        unit_order = list(range(len(units)))
        tie_breakers = {index: rng.random() for index in unit_order}
        unit_order.sort(key=lambda index: (len(candidate_rows_by_unit[index]), tie_breakers[index]))
        occupied_resources = set()
        group_periods = defaultdict(set)
        selected = {}
        score_total = 0.0

        for index in unit_order:
            if index % 128 == 0 and _lease_lost(cancel_event):
                return
            requirement, lesson_unit = units[index]
            group_keys = requirement.get("group_keys", [requirement["group"]])
            rows = []
            for key, variable, resource_tokens in candidate_rows_by_unit[index]:
                if any(token in occupied_resources for token in resource_tokens):
                    continue
                _, day, slot_key, _, week_type = key
                position = slot_order[slot_key]
                weeks = ("even", "odd") if week_type == "both" else (week_type,)
                score = placement_costs.cost(requirement, day, slot_key, week_type) * 10
                score += (position * 0.1) + rng.random() * 0.01
                for group_key in group_keys:
                    for week in weeks:
                        periods = group_periods[(group_key, week, day)]
                        if not periods:
                            score += 5
                        else:
                            if position - 1 in periods or position + 1 in periods:
                                score -= 2
                            if min(periods) < position < max(periods):
                                score -= 4
                rows.append((score, key, variable, resource_tokens, day, slot_key, weeks))

            if not rows:
                continue
            score, key, variable, resource_tokens, day, slot_key, weeks = min(rows, key=lambda row: row[0])
            selected[index] = (key, variable)
            score_total += score
            occupied_resources.update(resource_tokens)
            position = slot_order[slot_key]
            for group_key in group_keys:
                for week in weeks:
                    group_periods[(group_key, week, day)].add(position)

        if len(selected) > best_count or (len(selected) == best_count and score_total < best_score):
            best_choices = selected
            best_score = score_total
            best_count = len(selected)

    hinted_candidates = set()
    for index, assigned in enumerate(assigned_units):
        selected = best_choices.get(index)
        model.AddHint(assigned, int(selected is not None))
        if selected is not None:
            hinted_candidates.add(selected[1].Index())

    # Complete the placement hint so CP-SAT can start from the conflict-free
    # greedy schedule instead of repairing only the per-unit assigned flags.
    for rows in candidate_rows_by_unit:
        for _, variable, _ in rows:
            model.AddHint(variable, int(variable.Index() in hinted_candidates))


def _lease_lost(cancel_event):
    return cancel_event is not None and cancel_event.is_set()


def _lease_lost_result():
    return {"status": "unknown", "reason": "GENERATION_LEASE_LOST", "placements": [], "findings": []}


def _overlap_group_subgroups(subgroup):
    return (0, 1, 2) if subgroup == 0 else (subgroup,)


def _teachers(requirement):
    return requirement.get("teachers") or [requirement["teacher"]]
