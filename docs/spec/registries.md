# Wire and policy registries

Status: normative MMP/1 source

The reference values below are centralized in `mmp-types/src/consts.rs`. Each value has a scope and status; local policy values may be tightened but are not automatically wire-compatible when changed.

| Name | Value | Scope | Status |
|---|---:|---|---|
| MAX_BUNDLE | 4096 bytes | wire/safety | interoperability bound |
| MAX_FRAME_PAYLOAD | 4096 bytes | wire/safety | interoperability bound |
| L_MAX | 32 | local copy admission | safety bound |
| LIFETIME_CONTROL | 6 h | class 0 | wire policy |
| LIFETIME_MESSAGE | 72 h | classes 1,2,4,unknown | wire policy |
| LIFETIME_CLAIM | 7 d | class 3 | wire policy |
| QUOTA_CORE/STANDARD/ANCHOR | 32 KiB / 128 MiB / 2 GiB | local store | deployment policy |
| RBSR_T_LIST / RBSR_B_F | 24 / 16 | reconciliation | implementation policy |
| SYMBOL_SIZE | 2048 bytes | file extension | extension interoperability |
| RS_DATA_K / RS_TOTAL_N | 32 / 40 | file extension | extension interoperability |
| RATCHET_WINDOW | 512 | Standard local state | policy default |
| HANDSHAKE/IDLE/AIRTIME | 5 s / 10 s / 60 s | session timers | policy default |
| EPHEM_SLOT | 15 min | beacon rotation | policy default |
| P_TABLE/CONTACT_GRAPH | 256 / 4096 entries | local memory | safety bound |
| ORIGIN_BUCKET | 32 KiB/h, 128 KiB burst | local admission | safety bound |

Research values such as EDR activity rates, mobility distributions, and evidence-weighted spray parameters are not in this registry. Authority certificate fields and deployment-profile constants are extension or deployment values.
