// На какой системе открыто окно. Нужно интерфейсу для названий клавиш
// (на Mac — Option и Cmd) и для разделов, которые на системах разные.
export const isMac = /Mac/.test(navigator.userAgent)

export const ALT_NAME = isMac ? 'Option' : 'Alt'
export const WIN_NAME = isMac ? 'Cmd' : 'Win'
