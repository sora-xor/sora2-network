/*
 * Copyright D3 Ledger, Inc. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

@file:JvmName("BootstrapMain")

package jp.co.soramitsu.bootstrap

import com.fasterxml.jackson.module.kotlin.KotlinModule
import org.springframework.boot.SpringApplication
import org.springframework.boot.autoconfigure.SpringBootApplication
import org.springframework.context.annotation.Bean
import org.springframework.http.converter.json.Jackson2ObjectMapperBuilder

@SpringBootApplication
class Application

fun main(args: Array<String>) {
    val app = SpringApplication(Application::class.java)
    app.run(*args)
}

@Bean
fun objectMapperBuilder(): Jackson2ObjectMapperBuilder
        = Jackson2ObjectMapperBuilder().modulesToInstall(KotlinModule())
