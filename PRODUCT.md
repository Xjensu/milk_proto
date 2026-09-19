# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Two audiences:

- **Public, unauthenticated visitors** — students, staff, or anyone — looking up a class schedule by group, teacher, or room. No login required; self-serve lookup only.
- **Authenticated staff** who run the scheduling apparatus:
  - A registrar/dean's-office-style **dispatcher team** that builds the whole semester's schedule under time pressure at the start of each term, then makes reactive edits (teacher absence, room conflict, individual reschedules) during the term.
  - **Department-level admins** (`faculty_admin`, `kafedra_admin` roles), each scoped to their own faculty/department rather than the whole university.
  - **Teachers**, with self-service access to their own schedule and teaching load.
  - Account roles: `super_admin`, `faculty_admin`, `kafedra_admin`, `scheduler`, `teacher`, `student`.

## Product Purpose

Replaces manual, spreadsheet-based university class scheduling with one coordinated system: build and publish the semester schedule, manage teaching-load distribution across departments, track room and teacher availability, and handle mid-term reschedules — without double-booking a room or teacher — while giving anyone who just needs to find a class a fast public lookup with no login.

## Positioning

Automated, conflict-aware, recommendation-driven scheduling, as against a plain spreadsheet or generic calendar tool: the system proposes valid placements for a given group/day/slot from the load distribution and rejects double-bookings server-side (not after the fact), and ownership-scoped access lets each department manage its own slice of the schedule without funneling every change through one central administrator.

## Operating Context

- **Semester-start crunch**: a dispatcher/registrar team builds the term's schedule against defined load requirements before classes begin.
- **Ongoing reactive edits during the term**: individual class occurrences get moved, swapped, or cancelled (teacher absence, room conflict) without altering the underlying recurring weekly pattern.
- **Distributed department-level management**: faculty and department (kafedra) admins each work within their own scope rather than one central authority editing everything.
- **Calendar-governed terms**: a semester is organized into named periods — the regular teaching term, a lecture period, a credit period, and an exam session — each with its own date range that scheduling activity is checked against.

## Capabilities and Constraints

- Russian-language admin interface. No confirmed requirement for another language or locale at this time.
- No confirmed external system integration (no student-information-system, 1C, or accreditation-reporting link established).
- **Generic, not tied to one named institution** — the product is meant to be deployable at more than one institution, so product framing should stay institution-agnostic rather than committing to a specific university's identity.
- Recurring terminology to preserve: *факультет* (faculty), *кафедра* (department/chair), *поток* (stream — a joint-taught group of curriculum streams sharing a class), *нагрузка* (teaching load).

## Brand Commitments

Product name **Milk Schedule**, with the Russian tagline *"Ритм университета в одном экране"* ("The university's rhythm, in one screen"). No confirmed institution name, logo, or identity to preserve beyond the product's own brand — consistent with the generic/multi-institution positioning above.

## Evidence on Hand

None. The development database holds only test/seed fixtures (many records visibly prefixed "TEST", some scrambled reference data such as mismatched week-type name/code pairs). Future work must not treat this data as real content — there is no real testimonial, case study, or production dataset to draw from yet.

## Product Principles

- One coordinated source of truth for the schedule, replacing spreadsheets and ad hoc coordination.
- Prevent conflicts by construction: room/teacher double-booking is rejected server-side, not caught after the fact.
- Scope authority to ownership: each department/faculty admin manages their own slice; nobody needs blanket access just to do their job.
- Recommend, don't just record: surface valid placements for a cell instead of making the dispatcher check availability by hand.
- Every admin action should be fast enough for someone working under semester-start time pressure — minimize manual ID entry and raw-data dead ends in favor of visual pickers and readable labels.

## Accessibility & Inclusion

No product-specific accessibility requirement has been established.
