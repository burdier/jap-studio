# Jaybu Design System — Hay DAW
# Preset: Ableton Live Dark — Studio (acentos desaturados)
# Se puede copiar a ~/.jaybu.profile y recargar con R o :reload.
# FONT_SIZE_SM/BASE/LG, FONT_FAMILY, GAP_* y PADDING_MD se conservan
# como claves del perfil, pero la UI actual todavía no las consume.

# Palette
BG=282a2b
PANEL=333537
PANEL_HOVER=404345
PANEL_DARK=202223
BORDER=525659
TEXT=cbd0d1
MUTED=969da1
ACCENT=c0a16c
ACCENT_DIM=4b4233
SUCCESS=8cac91
WARNING=b9ad79
DANGER=c1837b
CYAN=7fa9a3
ORANGE=c19c83
PINK=a494b9
GRID=424649

# Layout dimensions
TRACK_HEADER_WIDTH=180
TRACK_HEADER_HEIGHT=58
TIMELINE_BAR_HEIGHT=24
TIMELINE_PIXELS_PER_BEAT=32
TITLE_BAR_HEIGHT=28
TRANSPORT_HEIGHT=36
STATUS_BAR_HEIGHT=22

# Control sizes
PAN_BAR_WIDTH=36
VOL_BAR_WIDTH=40
CONTROL_BAR_HEIGHT=5
TRANSPORT_BTN_SIZE=24

# Typography
FONT_SIZE_XS=12
FONT_SIZE_SM=11
FONT_SIZE_BASE=12
FONT_SIZE_LG=15
FONT_FAMILY=sans-serif

# Spacing
GAP_XS=3
GAP_SM=6
GAP_MD=8
GAP_LG=12
PADDING_XS=3
PADDING_SM=6
PADDING_MD=8
PADDING_LG=12

# Border radius
RADIUS_SM=1
RADIUS_MD=2
RADIUS_LG=3

# Theme
THEME=ableton-live-dark
CURSOR_FX=off
PLAYHEAD_PARTICLE_COLOR=8cac91
SELECTION_CONTRAST=0.22

# Export
EXPORT_DEFAULTS=export -n 'heydawexportingfirstwave.[wav]' -master -out 'exports' -sr 48000 -bits 24 -bitrate 320 -stereo -start 0 -length all -tail 0 -master-fx auto -tracks all
