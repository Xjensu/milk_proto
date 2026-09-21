#!/usr/bin/env python3
"""Build the screenshot-only administrator schedule workflow deck.

The deck intentionally contains no overlay text: the presenter narrates the
workflow while each slide shows one full-screen application capture.
"""

from __future__ import annotations

import shutil
import sys
import zipfile
from pathlib import Path
from xml.sax.saxutils import escape

from PIL import Image


ROOT = Path(__file__).resolve().parents[2]
OUT = Path(__file__).resolve().parent / "administrator_schedule_creation_workflow.pptx"
WORK = Path("/tmp/milk-schedule-presentation-build")

SLIDES = [
    ("schedule-context-1280.png", ROOT / "docs/design/admin-workbench-2026-09-13/evidence/schedule-context-1280.png"),
    ("teachers-1280.png", ROOT / "docs/design/admin-workbench-2026-09-13/evidence/teachers-1280.png"),
    ("workload-1280.png", ROOT / "docs/design/admin-workbench-2026-09-13/evidence/workload-1280.png"),
    ("workload-editor-1280.png", ROOT / "docs/design/admin-workbench-2026-09-13/evidence/workload-editor-1280.png"),
    ("schedule-grid-1280.png", ROOT / "docs/design/admin-workbench-2026-09-13/evidence/schedule-grid-1280.png"),
    ("weekday-transfers-1280.png", ROOT / "docs/design/admin-workbench-2026-09-13/evidence/weekday-transfers-1280.png"),
    ("reservations-1280.png", ROOT / "docs/design/admin-workbench-2026-09-13/evidence/reservations-1280.png"),
    ("excel-import-entry.png", ROOT / ".playwright-mcp/page-2026-09-20T09-33-15-572Z.png"),
    ("notice-1280.png", ROOT / "docs/design/admin-workbench-2026-09-13/evidence/notice-1280.png"),
    ("user-schedule.png", ROOT / "apps/frontend/qa-schedule-group.png"),
]

NS_A = "http://schemas.openxmlformats.org/drawingml/2006/main"
NS_R = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
NS_P = "http://schemas.openxmlformats.org/presentationml/2006/main"


def full_bleed(source: Path, destination: Path) -> None:
    """Crop an application screenshot to 16:9 without adding presentation UI."""
    image = Image.open(source).convert("RGB")
    target_ratio = 16 / 9
    current_ratio = image.width / image.height
    if current_ratio > target_ratio:
        width = round(image.height * target_ratio)
        left = (image.width - width) // 2
        image = image.crop((left, 0, left + width, image.height))
    else:
        height = round(image.width / target_ratio)
        image = image.crop((0, 0, image.width, min(height, image.height)))
    image.resize((1600, 900), Image.Resampling.LANCZOS).save(destination, format="PNG", optimize=True)


def slide_xml(image_name: str) -> str:
    return (
        f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<p:sld xmlns:a="{NS_A}" xmlns:r="{NS_R}" xmlns:p="{NS_P}">'
        '<p:cSld name="Screenshot"><p:spTree>'
        '<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>'
        '<p:grpSpPr/>'
        f'<p:pic><p:nvPicPr><p:cNvPr id="2" name="{escape(image_name)}"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr>'
        '<p:blipFill><a:blip r:embed="rId1"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>'
        '<p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="12192000" cy="6858000"/></a:xfrm>'
        '<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>'
        '</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>'
    )


def slide_rels(image_name: str) -> str:
    return (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        f'<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/{escape(image_name)}"/>'
        '<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>'
        '</Relationships>'
    )


def write_package() -> None:
    if WORK.exists():
        shutil.rmtree(WORK)
    for folder in ("_rels", "docProps", "ppt/_rels", "ppt/media", "ppt/slides", "ppt/slides/_rels", "ppt/slideLayouts", "ppt/slideLayouts/_rels", "ppt/slideMasters", "ppt/slideMasters/_rels", "ppt/theme"):
        (WORK / folder).mkdir(parents=True, exist_ok=True)

    media_names = []
    for name, source in SLIDES:
        if not source.exists():
            raise FileNotFoundError(source)
        media_name = name
        full_bleed(source, WORK / "ppt/media" / media_name)
        media_names.append(media_name)

    (WORK / "[Content_Types].xml").write_text(
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
        '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
        '<Default Extension="xml" ContentType="application/xml"/><Default Extension="png" ContentType="image/png"/>'
        '<Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>'
        '<Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>'
        '<Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>'
        '<Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>'
        + "".join(
            f'<Override PartName="/ppt/slides/slide{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>'
            for i in range(1, len(media_names) + 1)
        )
        + '</Types>',
        encoding="utf-8",
    )
    (WORK / "_rels/.rels").write_text(
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>'
        '</Relationships>', encoding="utf-8")
    (WORK / "docProps/core.xml").write_text(
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" '
        'xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/">'
        '<dc:title>Administrator schedule creation workflow</dc:title><dc:creator>Milk Schedule</dc:creator>'
        '<cp:lastModifiedBy>Milk Schedule</cp:lastModifiedBy></cp:coreProperties>', encoding="utf-8")
    (WORK / "docProps/app.xml").write_text(
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">'
        '<Application>Milk Schedule presentation builder</Application></Properties>', encoding="utf-8")

    slide_ids = "".join(f'<p:sldId id="{255 + i}" r:id="rId{i + 1}"/>' for i in range(1, len(media_names) + 1))
    (WORK / "ppt/presentation.xml").write_text(
        f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<p:presentation xmlns:a="{NS_A}" xmlns:r="{NS_R}" xmlns:p="{NS_P}">'
        '<p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst>'
        f'<p:sldIdLst>{slide_ids}</p:sldIdLst><p:sldSz cx="12192000" cy="6858000" type="screen16x9"/>'
        '<p:notesSz cx="6858000" cy="9144000"/></p:presentation>', encoding="utf-8")
    presentation_rels = (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>'
        + "".join(
            f'<Relationship Id="rId{i + 1}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{i}.xml"/>'
            for i in range(1, len(media_names) + 1)
        )
        + '</Relationships>'
    )
    (WORK / "ppt/_rels/presentation.xml.rels").write_text(presentation_rels, encoding="utf-8")

    (WORK / "ppt/slideLayouts/slideLayout1.xml").write_text(
        f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sldLayout xmlns:a="{NS_A}" xmlns:r="{NS_R}" xmlns:p="{NS_P}" type="blank">'
        '<p:cSld name="Blank"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld>'
        '<p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>', encoding="utf-8")
    (WORK / "ppt/slideLayouts/_rels/slideLayout1.xml.rels").write_text(
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>', encoding="utf-8")
    (WORK / "ppt/slideMasters/slideMaster1.xml").write_text(
        f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sldMaster xmlns:a="{NS_A}" xmlns:r="{NS_R}" xmlns:p="{NS_P}">'
        '<p:cSld name="Master"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld>'
        '<p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>'
        '<p:sldLayoutIdLst><p:sldLayoutId id="1" r:id="rId1"/></p:sldLayoutIdLst><p:txStyles/></p:sldMaster>', encoding="utf-8")
    (WORK / "ppt/slideMasters/_rels/slideMaster1.xml.rels").write_text(
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>'
        '<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/></Relationships>', encoding="utf-8")
    (WORK / "ppt/theme/theme1.xml").write_text(
        f'<?xml version="1.0" encoding="UTF-8" standalone="yes"?><a:theme xmlns:a="{NS_A}" name="Milk Schedule"><a:themeElements><a:clrScheme name="Office"><a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="1F1F1F"/></a:dk2><a:lt2><a:srgbClr val="F2F2F2"/></a:lt2><a:accent1><a:srgbClr val="5A2634"/></a:accent1><a:accent2><a:srgbClr val="DCE9B4"/></a:accent2><a:accent3><a:srgbClr val="B69A7E"/></a:accent3><a:accent4><a:srgbClr val="7F7F7F"/></a:accent4><a:accent5><a:srgbClr val="C0504D"/></a:accent5><a:accent6><a:srgbClr val="9BBB59"/></a:accent6><a:hlink><a:srgbClr val="0000FF"/></a:hlink><a:folHlink><a:srgbClr val="800080"/></a:folHlink></a:clrScheme><a:fontScheme name="Office"><a:majorFont><a:latin typeface="Arial"/></a:majorFont><a:minorFont><a:latin typeface="Arial"/></a:minorFont></a:fontScheme><a:fmtScheme name="Office"><a:fillStyleLst/><a:lnStyleLst/><a:effectStyleLst/><a:bgFillStyleLst/></a:fmtScheme></a:themeElements></a:theme>', encoding="utf-8")

    for index, image_name in enumerate(media_names, start=1):
        (WORK / f"ppt/slides/slide{index}.xml").write_text(slide_xml(image_name), encoding="utf-8")
        (WORK / f"ppt/slides/_rels/slide{index}.xml.rels").write_text(slide_rels(image_name), encoding="utf-8")

    OUT.unlink(missing_ok=True)
    with zipfile.ZipFile(OUT, "w", compression=zipfile.ZIP_DEFLATED) as package:
        for path in WORK.rglob("*"):
            if path.is_file():
                package.write(path, path.relative_to(WORK).as_posix())
    print(f"wrote {OUT} ({len(media_names)} slides)")


if __name__ == "__main__":
    try:
        write_package()
    except Exception as error:
        print(error, file=sys.stderr)
        raise
