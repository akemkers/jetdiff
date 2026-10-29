import type { Register } from 'claude-code'

export const register: Register = on => {
  on('session.start', async ($, e, next) => {
    await $.command.register({
      name: 'jetdiff',
      description: 'Open this repo in jetdiff',
      argumentHint: '[folder | commit | pr-link | left right]',
      immediate: true,
    })

    return next(e)
  })

  on('command.run', { command: 'jetdiff' }, async ($, e) => {
    const found = await $.process.run(['sh', '-c', 'command -v jetdiff'])
    if (found.exitCode !== 0) {
      return { text: 'jetdiff is not on PATH.' }
    }

    // Detached, so the window outlives a reload of the plugin and the session.
    const args = e.args.split(/\s+/).filter(Boolean)
    const launched = await $.process.run([
      'sh',
      '-c',
      'nohup "$@" >/dev/null 2>&1 &',
      'sh',
      'jetdiff',
      ...args,
    ])
    if (launched.exitCode !== 0) {
      return { text: `Could not start jetdiff: ${launched.stderr.trim()}` }
    }

    return { text: args.length === 0 ? 'Opened jetdiff.' : `Opened jetdiff ${args.join(' ')}.` }
  })
}
