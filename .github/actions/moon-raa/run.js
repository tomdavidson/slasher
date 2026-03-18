const { execSync } = require('child_process')
const lib = require('./lib')

const discoverProjects = core => {
  const { projects } = JSON.parse(execSync('moon query projects --affected').toString())

  const withRaa = projects.filter(p => p.config?.project?.metadata?.raa).map(p => ({
    id: p.id,
    raa: p.config.project.metadata.raa,
  }))

  return lib.partitionProjects(withRaa, core)
}

const runTasks = async (exec, core, projectEntries) => {
  const results = []
  for (const project of projectEntries) {
    const target = lib.targetName(project.id, project.run)
    core.info(`Running ${target}`)
    try {
      await exec.exec('moon', ['run', target])
      results.push({ ...project, status: 'done' })
    } catch (error) {
      core.error(`Failed ${target}: ${error.message}`)
      results.push({ ...project, status: 'failed' })
    }
  }
  return results
}

module.exports = async ({ github, context, core, exec, inputs }) => {
  const { owner, repo } = context.repo
  const { pr_number, prefix } = inputs
  const environment = lib.environmentName(prefix, pr_number)

  const valid = discoverProjects(core)

  if (!valid.length) {
    core.info('No affected projects with valid raa metadata')
    core.setOutput('deployment-id', '')
    core.setOutput('environment', environment)
    core.setOutput('project-count', '0')
    core.setOutput('summary', 'No RAA projects affected.')
    core.setOutput('manifest', '{}')
    return
  }

  const projectEntries = valid.map(lib.buildProjectEntry(prefix, pr_number))
  const manifest = lib.buildManifest(pr_number, projectEntries)

  await lib.deactivateDeployments(github, owner, repo, environment, core)

  const deployment = await lib.createDeployment(github, owner, repo, {
    ref: context.sha,
    environment,
    description: `Review for PR #${pr_number} (${valid.length} projects)`,
    payload: manifest,
  })

  await lib.setDeploymentStatus(
    github,
    owner,
    repo,
    deployment.id,
    'in_progress',
    'Building review environment...',
  )

  const results = await runTasks(exec, core, projectEntries)
  const allPassed = results.every(r => r.status === 'done')

  const state = allPassed ? 'success' : 'failure'
  const description = allPassed ? 'Review build complete' : 'Review build failed'
  await lib.setDeploymentStatus(github, owner, repo, deployment.id, state, description)

  const summary = lib.formatRunSummary(environment, results)
  core.setOutput('deployment-id', String(deployment.id))
  core.setOutput('environment', environment)
  core.setOutput('project-count', String(projectEntries.length))
  core.setOutput('summary', summary)
  core.setOutput('manifest', JSON.stringify(manifest))

  if (!allPassed) core.setFailed('One or more RAA tasks failed')
}
