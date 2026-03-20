const environmentName = (prefix, prNumber) => `${prefix}-pr-${prNumber}`

const artifactName = (prefix, projectId, prNumber) => `${prefix}-${projectId}-pr-${prNumber}`

const targetName = (projectId, task) => `${projectId}:${task}`

const buildManifest = (prNumber, projects) => ({ pr_number: prNumber, projects })

const buildProjectEntry = (prefix, prNumber) => project => ({
  id: project.id,
  artifact_name: artifactName(prefix, project.id, prNumber),
  run: project.raa.run,
  rm: project.raa.rm,
})

const validateProject = project => {
  const raa = project.raa
  const missing = [!raa && 'raa metadata', raa && !raa.run && 'raa.run', raa && !raa.rm && 'raa.rm'].filter(
    Boolean,
  )

  return missing.length ? { valid: false, reason: `missing ${missing.join(', ')}` } : { valid: true }
}

const partitionProjects = (projects, core) => {
  const valid = []
  for (const project of projects) {
    const result = validateProject(project)
    if (!result.valid) {
      core.warning(`Skipping ${project.id}: ${result.reason}`)
      continue
    }
    valid.push(project)
  }
  return valid
}

const formatRunSummary = (environment, projects) => {
  if (!projects.length) return 'No RAA projects affected.'

  const rows = projects.map(p => `| ${p.id} | \`${p.run}\` | \`${p.artifact_name}\` |`).join('\n')

  return [
    `### RAA Review`,
    '',
    '| Project | Task | Artifact |',
    '|---------|------|----------|',
    rows,
    '',
    `${projects.length} project(s) deployed to \`${environment}\``,
  ].join('\n')
}

const formatRmSummary = (environment, results) => {
  if (!results.length) return 'No RAA projects to tear down.'

  const rows = results.map(r => `| ${r.id} | \`${r.rm}\` | ${r.status} |`).join('\n')

  return [
    `### RAA Teardown`,
    '',
    '| Project | Task | Status |',
    '|---------|------|--------|',
    rows,
    '',
    `${results.length} project(s) torn down from \`${environment}\``,
  ].join('\n')
}

const listDeployments = async (github, owner, repo, environment) => {
  const { data } = await github.rest.repos.listDeployments({ owner, repo, environment })
  return data
}

const deactivateDeployments = async (github, owner, repo, environment, core) => {
  const deployments = await listDeployments(github, owner, repo, environment)

  for (const deployment of deployments) {
    await github.rest.repos.createDeploymentStatus({
      owner,
      repo,
      deployment_id: deployment.id,
      state: 'inactive',
    })
    await github.rest.repos.deleteDeployment({ owner, repo, deployment_id: deployment.id })
    core.info(`Deactivated and deleted deployment ${deployment.id}`)
  }

  return deployments
}

const createDeployment = async (github, owner, repo, { ref, environment, description, payload }) => {
  const { data: deployment } = await github.rest.repos.createDeployment({
    owner,
    repo,
    ref,
    environment,
    auto_merge: false,
    required_contexts: [],
    description,
    payload: JSON.stringify(payload),
  })
  return deployment
}

const setDeploymentStatus = async (github, owner, repo, deploymentId, state, description) => {
  await github.rest.repos.createDeploymentStatus({
    owner,
    repo,
    deployment_id: deploymentId,
    state,
    description: description ?? '',
  })
}

const deleteEnvironment = async (github, owner, repo, environment) => {
  try {
    await github.rest.repos.deleteAnEnvironment({ owner, repo, environment_name: environment })
  } catch (error) {
    if (error.status !== 404) throw error
  }
}

module.exports = {
  environmentName,
  artifactName,
  targetName,
  buildManifest,
  buildProjectEntry,
  validateProject,
  partitionProjects,
  formatRunSummary,
  formatRmSummary,
  listDeployments,
  deactivateDeployments,
  createDeployment,
  setDeploymentStatus,
  deleteEnvironment,
}
