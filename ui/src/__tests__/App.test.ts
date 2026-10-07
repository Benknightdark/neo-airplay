import { mount } from '@vue/test-utils'
import { createPinia } from 'pinia'
import { describe, expect, it } from 'vitest'
import App from '../App.vue'
import { router } from '../router'

describe('application shell', () => {
  it('renders the product heading and primary navigation', async () => {
    await router.push('/')
    await router.isReady()
    const wrapper = mount(App, { global: { plugins: [createPinia(), router] } })

    expect(wrapper.get('h1').text()).toBe('Neo AirPlay')
    expect(wrapper.get('[data-testid="nav-settings"]').text()).toContain('設定')
  })
})
