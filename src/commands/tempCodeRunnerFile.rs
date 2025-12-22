  run_pre_command_hooks(&mut command_hooks_context, &mut parsed_args, repository);
        let pre_command_duration = pre_command_start.elapsed();

        let git_start = Instant::now();
        //执行git
        let exit_status = proxy_to_git(&parsed_args.to_invocation_vec(), false);
        let git_duration = git_start.elapsed();
        //post
        let post_command_start = Instant::now();
        run_post_command_hooks(
            &mut command_hooks_context,
            &parsed_args,
            exit_status,
            repository,
        );
        let post_command_duration = post_command_start.elapsed();