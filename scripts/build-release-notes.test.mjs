import { describe, expect, it } from 'vitest'
import { buildBody } from './build-release-notes.mjs'

const changelog = `# Changelog

## [0.13.0] - 2026-07-10

### Highlights / 亮点

- 回退时标偏移检测 / Roll back clock-offset monitoring.

## [0.12.1] - 2026-06-29

- Previous release.
`

describe('buildBody', () => {
  it('renders the requested release and only its changelog section', () => {
    const body = buildBody('v0.13.0', changelog)

    expect(body).toContain('# PmuSim v0.13.0')
    expect(body).toContain('Roll back clock-offset monitoring.')
    expect(body).not.toContain('Previous release.')
  })

  it('lists versioned assets for both applications', () => {
    const body = buildBody('v0.13.0', changelog)

    expect(body).toContain('`PmuSim_0.13.0_aarch64.dmg`')
    expect(body).toContain('`PmuSim_0.13.0_x64-setup.exe`')
    expect(body).toContain('`PmuSub_0.13.0_aarch64.dmg`')
    expect(body).toContain('`PmuSub_0.13.0_amd64.AppImage`')
  })

  it('shows an explicit warning when the changelog section is missing', () => {
    const body = buildBody('v9.9.9', changelog)

    expect(body).toContain('CHANGELOG.md 缺少 `9.9.9` 的 section')
  })
})
