import path from 'node:path'
import { fileURLToPath } from 'node:url'

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const executableName = process.platform === 'win32' ? 'neo-airplay.exe' : 'neo-airplay'
const appBinaryPath = path.join(repositoryRoot, 'target', 'debug', executableName)
const profilePath = process.env.NEO_E2E_PROFILE

if (!profilePath) {
  throw new Error('NEO_E2E_PROFILE must point to an isolated test profile directory.')
}

const appArgs = ['--e2e-profile', profilePath]

export const config = {
  runner: 'local',
  specs: ['./specs/**/*.e2e.mjs'],
  maxInstances: 1,
  logLevel: 'error',
  bail: 1,
  waitforTimeout: 10000,
  connectionRetryTimeout: 90000,
  connectionRetryCount: 1,
  services: [
    [
      '@wdio/tauri-service',
      {
        appBinaryPath,
        appArgs,
        driverProvider: 'embedded',
        statusPollTimeout: 10000,
        captureBackendLogs: true,
        captureFrontendLogs: true,
      },
    ],
  ],
  capabilities: [
    {
      browserName: 'tauri',
      'tauri:options': {
        application: appBinaryPath,
        args: appArgs,
      },
    },
  ],
  framework: 'mocha',
  reporters: ['spec'],
  mochaOpts: {
    ui: 'bdd',
    timeout: 60000,
  },
  baseUrl: 'http://localhost',
}
